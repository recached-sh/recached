// sysc: an LD_PRELOAD shim that counts a server's I/O system calls and the
// time spent inside them, for hosts with no strace, perf or bpftrace
// (Phase 6O-B; perf_event_paranoid=2 rules out syscall tracepoints).
//
// It interposes the glibc wrappers the servers call (Recached, Redis and
// Valkey all import them dynamically): read write recv send readv writev
// epoll_wait epoll_ctl accept4, and syscall(2) itself (Rust's std issues
// futex through it). For each one it counts calls, the sum of positive
// returns (bytes, or events for epoll_wait), EAGAIN, zero returns (EOF, or an
// epoll_wait that timed out), other errors, and TSC cycles between entry and
// return. Those cycles are the kernel's time in the call *including* any
// softirq work the kernel ran inline before returning (loopback TCP delivers
// the peer's packet in the sender's context), plus about 40 cycles of shim.
//
// Counters live in a file mapped MAP_SHARED (SYSC_OUT=<path>), so a reader
// can snapshot them before and after a timed run while the server keeps
// running; see sysc_read.py. Without SYSC_OUT the shim counts nothing.
//
// It is a diagnostic: the throughput cells of the kit never load it.
//
// Build: gcc -O2 -Wall -shared -fPIC -o sysc.so sysc.c -ldl
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/epoll.h>
#include <sys/mman.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <sys/uio.h>
#include <time.h>
#include <unistd.h>
#include <x86intrin.h>

enum {
    S_READ, S_WRITE, S_RECV, S_SEND, S_READV, S_WRITEV,
    S_EPOLL_WAIT, S_EPOLL_WAIT_T0, S_EPOLL_CTL, S_FUTEX, S_SYSCALL_OTHER, S_ACCEPT4,
    S_N
};

struct slot { uint64_t calls, pos_sum, eagain, zero, err, tsc; };
struct shared {
    uint64_t magic;      // "SYSC6OB1"
    uint64_t version;    // 1
    uint64_t tsc_hz;
    uint64_t pid;
    struct slot s[S_N];
};

static struct shared *sh;

static ssize_t (*r_read)(int, void *, size_t);
static ssize_t (*r_write)(int, const void *, size_t);
static ssize_t (*r_recv)(int, void *, size_t, int);
static ssize_t (*r_send)(int, const void *, size_t, int);
static ssize_t (*r_readv)(int, const struct iovec *, int);
static ssize_t (*r_writev)(int, const struct iovec *, int);
static int (*r_epoll_wait)(int, struct epoll_event *, int, int);
static int (*r_epoll_ctl)(int, int, int, struct epoll_event *);
static long (*r_syscall)(long, ...);
static int (*r_accept4)(int, struct sockaddr *, socklen_t *, int);

static uint64_t now_ns(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC_RAW, &t);
    return (uint64_t)t.tv_sec * 1000000000u + (uint64_t)t.tv_nsec;
}

__attribute__((constructor)) static void sysc_init(void) {
    r_read = dlsym(RTLD_NEXT, "read");
    r_write = dlsym(RTLD_NEXT, "write");
    r_recv = dlsym(RTLD_NEXT, "recv");
    r_send = dlsym(RTLD_NEXT, "send");
    r_readv = dlsym(RTLD_NEXT, "readv");
    r_writev = dlsym(RTLD_NEXT, "writev");
    r_epoll_wait = dlsym(RTLD_NEXT, "epoll_wait");
    r_epoll_ctl = dlsym(RTLD_NEXT, "epoll_ctl");
    r_syscall = dlsym(RTLD_NEXT, "syscall");
    r_accept4 = dlsym(RTLD_NEXT, "accept4");
    const char *path = getenv("SYSC_OUT");
    if (!path || !*path) return;
    // The real syscall, not the wrappers above: they would count themselves.
    int fd = (int)r_syscall(SYS_openat, AT_FDCWD, path, O_RDWR | O_CREAT | O_TRUNC | O_CLOEXEC, 0644);
    if (fd < 0) return;
    if (ftruncate(fd, sizeof(struct shared)) != 0) { r_syscall(SYS_close, fd); return; }
    void *m = mmap(NULL, sizeof(struct shared), PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    r_syscall(SYS_close, fd);
    if (m == MAP_FAILED) return;
    struct shared *s = m;
    memset(s, 0, sizeof *s);
    // TSC frequency against CLOCK_MONOTONIC_RAW over a 50 ms spin.
    uint64_t n0 = now_ns(), t0 = __rdtsc(), n1;
    do { n1 = now_ns(); } while (n1 - n0 < 50000000u);
    uint64_t t1 = __rdtsc();
    s->tsc_hz = (uint64_t)((double)(t1 - t0) * 1e9 / (double)(n1 - n0));
    s->pid = (uint64_t)getpid();
    s->version = 1;
    memcpy(&s->magic, "SYSC6OB1", 8);
    __atomic_thread_fence(__ATOMIC_SEQ_CST);
    sh = s;
}

static inline void note(int k, long ret, int err, uint64_t t0) {
    uint64_t dt = __rdtsc() - t0;
    struct slot *s = &sh->s[k];
    __atomic_fetch_add(&s->calls, 1, __ATOMIC_RELAXED);
    __atomic_fetch_add(&s->tsc, dt, __ATOMIC_RELAXED);
    if (ret > 0) __atomic_fetch_add(&s->pos_sum, (uint64_t)ret, __ATOMIC_RELAXED);
    else if (ret == 0) __atomic_fetch_add(&s->zero, 1, __ATOMIC_RELAXED);
    else if (err == EAGAIN) __atomic_fetch_add(&s->eagain, 1, __ATOMIC_RELAXED);
    else __atomic_fetch_add(&s->err, 1, __ATOMIC_RELAXED);
}

#define WRAP(k, call)                                            \
    do {                                                         \
        if (!sh) return call;                                    \
        uint64_t t0 = __rdtsc();                                 \
        __typeof__(call) ret = call;                             \
        int e = errno;                                           \
        note(k, (long)ret, e, t0);                               \
        errno = e;                                               \
        return ret;                                              \
    } while (0)

ssize_t read(int fd, void *b, size_t n) { WRAP(S_READ, r_read(fd, b, n)); }
ssize_t write(int fd, const void *b, size_t n) { WRAP(S_WRITE, r_write(fd, b, n)); }
ssize_t recv(int fd, void *b, size_t n, int f) { WRAP(S_RECV, r_recv(fd, b, n, f)); }
ssize_t send(int fd, const void *b, size_t n, int f) { WRAP(S_SEND, r_send(fd, b, n, f)); }
ssize_t readv(int fd, const struct iovec *v, int c) { WRAP(S_READV, r_readv(fd, v, c)); }
ssize_t writev(int fd, const struct iovec *v, int c) { WRAP(S_WRITEV, r_writev(fd, v, c)); }
int epoll_ctl(int ep, int op, int fd, struct epoll_event *ev) { WRAP(S_EPOLL_CTL, r_epoll_ctl(ep, op, fd, ev)); }
int accept4(int fd, struct sockaddr *a, socklen_t *l, int f) { WRAP(S_ACCEPT4, r_accept4(fd, a, l, f)); }
int epoll_wait(int ep, struct epoll_event *ev, int max, int timeout) {
    WRAP(timeout == 0 ? S_EPOLL_WAIT_T0 : S_EPOLL_WAIT, r_epoll_wait(ep, ev, max, timeout));
}

long syscall(long nr, ...) {
    va_list ap;
    va_start(ap, nr);
    long a = va_arg(ap, long), b = va_arg(ap, long), c = va_arg(ap, long);
    long d = va_arg(ap, long), e = va_arg(ap, long), f = va_arg(ap, long);
    va_end(ap);
    WRAP(nr == SYS_futex ? S_FUTEX : S_SYSCALL_OTHER, r_syscall(nr, a, b, c, d, e, f));
}
