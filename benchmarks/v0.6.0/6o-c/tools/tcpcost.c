// tcpcost: the kernel cost of one small loopback TCP reply, by syscall kind.
//
// Phase 6O-B. Recached replies with send(2) and reads with recv(2) (Rust's
// std); Redis and Valkey use write(2) and read(2). The sysc.so shim measured
// Recached's send at ~4.06 us per call against Redis's write at ~3.65 us for
// the same 5-byte reply. This isolates the syscall kind from everything else:
// one thread, one connected loopback TCP pair with TCP_NODELAY, the peer
// never asleep (it is the same thread), so no wake-up is involved.
//
//   tcpcost <iterations> <payload bytes>
//
// For each of {send, write} x {recv, read} it times ITER rounds of
// "reply on A, read it on B" and prints ns per reply call and per read call.
// The rounds alternate kinds in blocks so drift lands on all of them.
//
// Build: gcc -O2 -Wall -o tcpcost tcpcost.c
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <time.h>
#include <unistd.h>
#include <x86intrin.h>

static int pair(int *a, int *b) {
    int l = socket(AF_INET, SOCK_STREAM, 0), one = 1;
    struct sockaddr_in sa = {.sin_family = AF_INET, .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    socklen_t len = sizeof sa;
    if (bind(l, (void *)&sa, sizeof sa) || listen(l, 1) || getsockname(l, (void *)&sa, &len)) return -1;
    *a = socket(AF_INET, SOCK_STREAM, 0);
    if (connect(*a, (void *)&sa, sizeof sa)) return -1;
    *b = accept(l, NULL, NULL);
    close(l);
    setsockopt(*a, IPPROTO_TCP, TCP_NODELAY, &one, sizeof one);
    setsockopt(*b, IPPROTO_TCP, TCP_NODELAY, &one, sizeof one);
    return 0;
}

static double hz(void) {
    struct timespec t0, t1;
    clock_gettime(CLOCK_MONOTONIC_RAW, &t0);
    uint64_t c0 = __rdtsc();
    do clock_gettime(CLOCK_MONOTONIC_RAW, &t1); while ((t1.tv_sec - t0.tv_sec) * 1e9 + (t1.tv_nsec - t0.tv_nsec) < 5e7);
    return (double)(__rdtsc() - c0) * 1e9 / ((t1.tv_sec - t0.tv_sec) * 1e9 + (t1.tv_nsec - t0.tv_nsec));
}

int main(int argc, char **argv) {
    long iters = argc > 1 ? atol(argv[1]) : 1000000;
    size_t n = argc > 2 ? (size_t)atol(argv[2]) : 5;
    char out[4096], in[16384];
    memset(out, '+', sizeof out);
    int a, b;
    if (pair(&a, &b)) { perror("pair"); return 1; }
    double f = hz();
    const char *names[4] = {"send+recv", "write+recv", "send+read", "write+read"};
    uint64_t tw[4] = {0}, tr[4] = {0}, cnt[4] = {0};
    const long block = 10000;
    for (long done = 0; done < iters; done += block) {
        for (int k = 0; k < 4; k++) {
            for (long i = 0; i < block; i++) {
                uint64_t t0 = __rdtsc();
                ssize_t w = (k & 1) ? write(a, out, n) : send(a, out, n, MSG_NOSIGNAL);
                uint64_t t1 = __rdtsc();
                ssize_t r = (k & 2) ? read(b, in, sizeof in) : recv(b, in, sizeof in, 0);
                uint64_t t2 = __rdtsc();
                if (w != (ssize_t)n || r != (ssize_t)n) { fprintf(stderr, "short %zd %zd\n", w, r); return 1; }
                tw[k] += t1 - t0; tr[k] += t2 - t1; cnt[k]++;
            }
        }
    }
    printf("payload=%zu iterations_per_kind=%lu tsc_hz=%.0f\n", n, (unsigned long)cnt[0], f);
    printf("%-11s %12s %12s\n", "kind", "reply ns", "read ns");
    for (int k = 0; k < 4; k++)
        printf("%-11s %12.1f %12.1f\n", names[k], tw[k] * 1e9 / f / cnt[k], tr[k] * 1e9 / f / cnt[k]);
    return 0;
}
