// uprof: unprivileged user-space CPU counters and samples for one process.
//
// Phase 6N-C5 replacement for `perf stat` / `perf record` on a host where the
// perf tool is not installed and kernel.perf_event_paranoid=2. Paranoid 2
// still lets a user open perf_event_open(2) events on their *own* processes
// as long as kernel and hypervisor execution are excluded, which is all this
// tool asks for: every event sets exclude_kernel=1 and exclude_hv=1. Nothing
// is installed and no sysctl is changed.
//
// Kernel time (syscalls, the loopback TCP stack, softirqs) is therefore NOT
// in these numbers; it is the process's stime in /proc, recorded separately.
//
//   uprof stat   <pid>                     count until SIGINT/SIGTERM, then print
//   uprof record <pid> <freq> <out> [lbr]  sample user IPs (+ call chains) until
//                                          SIGINT/SIGTERM; `lbr` uses the LBR
//                                          call stack instead of frame pointers
//
// Threads are enumerated from /proc/<pid>/task when the tool starts; tokio's
// workers exist from startup. A thread created later is not counted (the
// blocking pool, which the kit's AOF modes do not use under everysec).
//
// Build: gcc -O2 -Wall -o uprof uprof.c
#define _GNU_SOURCE
#include <dirent.h>
#include <errno.h>
#include <linux/perf_event.h>
#include <poll.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <unistd.h>

#define MAX_THREADS 256
#define RING_PAGES 64 /* data pages per sampling ring, a power of two */

static volatile sig_atomic_t stop;
static void on_signal(int sig) { (void)sig; stop = 1; }

static long peo(struct perf_event_attr *a, pid_t tid, int group) {
    return syscall(SYS_perf_event_open, a, tid, -1, group, PERF_FLAG_FD_CLOEXEC);
}

static int threads_of(pid_t pid, pid_t *out) {
    char path[64];
    snprintf(path, sizeof path, "/proc/%d/task", pid);
    DIR *d = opendir(path);
    if (!d) { perror(path); exit(2); }
    int n = 0;
    struct dirent *e;
    while ((e = readdir(d)) && n < MAX_THREADS)
        if (e->d_name[0] != '.') out[n++] = atoi(e->d_name);
    closedir(d);
    return n;
}

struct ev { const char *name; uint32_t type; uint64_t config; };
static const struct ev EVENTS[] = {
    {"cycles", PERF_TYPE_HARDWARE, PERF_COUNT_HW_CPU_CYCLES},
    {"instructions", PERF_TYPE_HARDWARE, PERF_COUNT_HW_INSTRUCTIONS},
    {"branch_misses", PERF_TYPE_HARDWARE, PERF_COUNT_HW_BRANCH_MISSES},
    {"cache_misses", PERF_TYPE_HARDWARE, PERF_COUNT_HW_CACHE_MISSES},
    {"l1d_read_misses", PERF_TYPE_HW_CACHE,
     PERF_COUNT_HW_CACHE_L1D | (PERF_COUNT_HW_CACHE_OP_READ << 8) |
         (PERF_COUNT_HW_CACHE_RESULT_MISS << 16)},
    {"task_clock_ns", PERF_TYPE_SOFTWARE, PERF_COUNT_SW_TASK_CLOCK},
    {"context_switches", PERF_TYPE_SOFTWARE, PERF_COUNT_SW_CONTEXT_SWITCHES},
};
#define NEV (sizeof EVENTS / sizeof EVENTS[0])

static int do_stat(pid_t pid) {
    pid_t tids[MAX_THREADS];
    int nt = threads_of(pid, tids);
    static int fd[MAX_THREADS][NEV];
    for (int t = 0; t < nt; t++)
        for (size_t e = 0; e < NEV; e++) {
            struct perf_event_attr a = {0};
            a.size = sizeof a;
            a.type = EVENTS[e].type;
            a.config = EVENTS[e].config;
            a.exclude_kernel = 1;
            a.exclude_hv = 1;
            a.read_format = PERF_FORMAT_TOTAL_TIME_ENABLED | PERF_FORMAT_TOTAL_TIME_RUNNING;
            fd[t][e] = (int)peo(&a, tids[t], -1);
            if (fd[t][e] < 0 && t == 0)
                fprintf(stderr, "uprof: %s on tid %d: %s\n", EVENTS[e].name, tids[t], strerror(errno));
        }
    signal(SIGINT, on_signal);
    signal(SIGTERM, on_signal);
    printf("uprof_threads=%d\n", nt);
    fflush(stdout);
    while (!stop) pause();
    for (size_t e = 0; e < NEV; e++) {
        double total = 0, run_ratio_min = 1;
        int opened = 0;
        for (int t = 0; t < nt; t++) {
            if (fd[t][e] < 0) continue;
            uint64_t v[3];
            if (read(fd[t][e], v, sizeof v) != sizeof v) continue;
            opened++;
            double scaled = v[0];
            if (v[2] && v[2] < v[1]) { scaled = (double)v[0] * v[1] / v[2]; }
            if (v[1] && (double)v[2] / v[1] < run_ratio_min) run_ratio_min = (double)v[2] / v[1];
            total += scaled;
        }
        printf("%s=%.0f opened=%d min_running=%.3f\n", EVENTS[e].name, total, opened, run_ratio_min);
    }
    return 0;
}

struct ring { int fd; pid_t tid; struct perf_event_mmap_page *meta; char *data; uint64_t mask; };

static void dump_maps(pid_t pid, const char *out) {
    char src[64], dst[4096];
    snprintf(src, sizeof src, "/proc/%d/maps", pid);
    snprintf(dst, sizeof dst, "%s.maps", out);
    FILE *in = fopen(src, "r"), *o = fopen(dst, "w");
    if (!in || !o) { perror("maps"); exit(2); }
    char buf[4096];
    size_t n;
    while ((n = fread(buf, 1, sizeof buf, in)) > 0) fwrite(buf, 1, n, o);
    fclose(in);
    fclose(o);
}

// Copy `len` bytes at ring position `pos` (wrapping) into dst.
static void ring_copy(struct ring *r, uint64_t pos, void *dst, size_t len) {
    for (size_t i = 0; i < len; i++) ((char *)dst)[i] = r->data[(pos + i) & r->mask];
}

static uint64_t lost_total, samples_total;

static void drain(struct ring *r, FILE *o) {
    uint64_t head = __atomic_load_n(&r->meta->data_head, __ATOMIC_ACQUIRE);
    uint64_t tail = r->meta->data_tail;
    static unsigned char rec[65536];
    while (tail < head) {
        struct perf_event_header h;
        ring_copy(r, tail, &h, sizeof h);
        if (h.size < sizeof h) break;
        ring_copy(r, tail, rec, h.size);
        unsigned char *p = rec + sizeof h;
        if (h.type == PERF_RECORD_SAMPLE) {
            uint64_t ip; uint32_t pid_, tid_; uint64_t nr;
            memcpy(&ip, p, 8); p += 8;
            memcpy(&pid_, p, 4); memcpy(&tid_, p + 4, 4); p += 8;
            memcpy(&nr, p, 8); p += 8;
            fprintf(o, "S %u %lx", tid_, (unsigned long)ip);
            for (uint64_t i = 0; i < nr; i++) {
                uint64_t a; memcpy(&a, p + 8 * i, 8);
                if (a >= (uint64_t)PERF_CONTEXT_MAX) continue; /* context markers */
                fprintf(o, " %lx", (unsigned long)a);
            }
            p += 8 * nr;
            if (r->meta->capabilities & 0) {}
            // Branch stack (LBR call stack mode): from addresses of open calls.
            if ((unsigned char *)p < rec + h.size) {
                uint64_t bnr; memcpy(&bnr, p, 8); p += 8;
                if (bnr) {
                    fprintf(o, " |");
                    for (uint64_t i = 0; i < bnr; i++) {
                        uint64_t from; memcpy(&from, p + 24 * i, 8);
                        fprintf(o, " %lx", (unsigned long)from);
                    }
                }
            }
            fputc('\n', o);
            samples_total++;
        } else if (h.type == PERF_RECORD_LOST) {
            uint64_t lost; memcpy(&lost, p + 8, 8);
            lost_total += lost;
        }
        tail += h.size;
    }
    __atomic_store_n(&r->meta->data_tail, tail, __ATOMIC_RELEASE);
}

static int do_record(pid_t pid, int freq, const char *out, int lbr) {
    pid_t tids[MAX_THREADS];
    int nt = threads_of(pid, tids);
    dump_maps(pid, out);
    FILE *o = fopen(out, "w");
    if (!o) { perror(out); return 2; }
    static struct ring rings[MAX_THREADS];
    struct pollfd pfd[MAX_THREADS];
    long page = sysconf(_SC_PAGESIZE);
    int nr = 0;
    for (int t = 0; t < nt; t++) {
        struct perf_event_attr a = {0};
        a.size = sizeof a;
        a.type = PERF_TYPE_HARDWARE;
        a.config = PERF_COUNT_HW_CPU_CYCLES;
        a.freq = 1;
        a.sample_freq = freq;
        a.sample_type = PERF_SAMPLE_IP | PERF_SAMPLE_TID | PERF_SAMPLE_CALLCHAIN;
        a.exclude_kernel = 1;
        a.exclude_hv = 1;
        a.exclude_callchain_kernel = 1;
        a.wakeup_events = 64;
        if (lbr) {
            a.sample_type |= PERF_SAMPLE_BRANCH_STACK;
            a.branch_sample_type = PERF_SAMPLE_BRANCH_USER | PERF_SAMPLE_BRANCH_CALL_STACK;
        }
        int fd = (int)peo(&a, tids[t], -1);
        if (fd < 0) { fprintf(stderr, "uprof: sample tid %d: %s\n", tids[t], strerror(errno)); continue; }
        size_t len = (size_t)(RING_PAGES + 1) * page;
        void *m = mmap(NULL, len, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
        if (m == MAP_FAILED) { perror("mmap"); close(fd); continue; }
        rings[nr] = (struct ring){fd, tids[t], m, (char *)m + page, (uint64_t)RING_PAGES * page - 1};
        pfd[nr] = (struct pollfd){fd, POLLIN, 0};
        nr++;
    }
    signal(SIGINT, on_signal);
    signal(SIGTERM, on_signal);
    fprintf(stderr, "uprof: sampling %d of %d threads at %d Hz%s\n", nr, nt, freq, lbr ? " (LBR call stack)" : "");
    while (!stop) {
        poll(pfd, nr, 100);
        for (int i = 0; i < nr; i++) drain(&rings[i], o);
    }
    for (int i = 0; i < nr; i++) drain(&rings[i], o);
    fclose(o);
    printf("uprof_samples=%lu lost=%lu threads=%d\n", (unsigned long)samples_total, (unsigned long)lost_total, nr);
    return 0;
}

int main(int argc, char **argv) {
    if (argc >= 3 && !strcmp(argv[1], "stat")) return do_stat(atoi(argv[2]));
    if (argc >= 5 && !strcmp(argv[1], "record"))
        return do_record(atoi(argv[2]), atoi(argv[3]), argv[4], argc >= 6 && !strcmp(argv[5], "lbr"));
    fprintf(stderr, "usage: uprof stat <pid> | uprof record <pid> <freq> <out> [lbr]\n");
    return 2;
}
