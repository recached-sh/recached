samples: 26087, user cycles/op 11942

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  14.1% |    1682 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   2.5% |     303 |
| connection (decode, reply, socket) | tokio mpsc |   2.2% |     266 |
| connection (decode, reply, socket) | tokio io/split |   1.6% |     188 |
| connection (decode, reply, socket) | tokio wake/schedule |   0.8% |     101 |
| connection (decode, reply, socket) | allocator |   0.6% |      67 |
| **connection (decode, reply, socket)** | **all** | ** 21.8%** | **   2607** |
| store execute | code |  12.7% |    1522 |
| store execute | libc (memcpy, syscall wrappers) |   5.6% |     666 |
| store execute | allocator |   0.2% |      22 |
| **store execute** | **all** | ** 18.5%** | **   2209** |
| frame build (broadcast_for_at) | allocator |   7.8% |     933 |
| frame build (broadcast_for_at) | code |   7.1% |     843 |
| frame build (broadcast_for_at) | libc (memcpy, syscall wrappers) |   1.2% |     145 |
| **frame build (broadcast_for_at)** | **all** | ** 16.1%** | **   1921** |
| tokio scheduler | tokio wake/schedule |   9.8% |    1166 |
| tokio scheduler | code |   2.8% |     340 |
| tokio scheduler | tokio io/split |   1.0% |     123 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.2% |      29 |
| tokio scheduler | atomics/Arc drop |   0.0% |       0 |
| **tokio scheduler** | **all** | ** 13.9%** | **   1658** |
| AOF append/lead/gate | code |   3.5% |     423 |
| AOF append/lead/gate | tokio io/split |   1.8% |     220 |
| AOF append/lead/gate | tokio wake/schedule |   1.6% |     192 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   1.3% |     160 |
| AOF append/lead/gate | allocator |   0.1% |       9 |
| AOF append/lead/gate | atomics/Arc drop |   0.0% |       1 |
| **AOF append/lead/gate** | **all** | **  8.4%** | **   1006** |
| ordered write path (rest) | code |   6.6% |     789 |
| ordered write path (rest) | tokio mpsc |   0.6% |      72 |
| ordered write path (rest) | libc (memcpy, syscall wrappers) |   0.4% |      53 |
| ordered write path (rest) | allocator |   0.1% |      11 |
| **ordered write path (rest)** | **all** | **  7.7%** | **    925** |
| on_write_logged (frame copy, dispatch) | code |   3.8% |     449 |
| on_write_logged (frame copy, dispatch) | libc (memcpy, syscall wrappers) |   1.3% |     155 |
| on_write_logged (frame copy, dispatch) | allocator |   0.9% |     107 |
| **on_write_logged (frame copy, dispatch)** | **all** | **  6.0%** | **    711** |
| stripe locks | code |   2.9% |     342 |
| stripe locks | tokio mpsc |   1.0% |     124 |
| **stripe locks** | **all** | **  3.9%** | **    466** |
| repl fan_out: backlog, offset, lock | code |   2.7% |     325 |
| repl fan_out: backlog, offset, lock | allocator |   0.3% |      30 |
| repl fan_out: backlog, offset, lock | atomics/Arc drop |   0.2% |      29 |
| repl fan_out: backlog, offset, lock | libc (memcpy, syscall wrappers) |   0.0% |       1 |
| **repl fan_out: backlog, offset, lock** | **all** | **  3.2%** | **    386** |
| tokio io driver | code |   0.3% |      39 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.1% |      14 |
| **tokio io driver** | **all** | **  0.4%** | **     54** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|   8.3%    987.9 | `libc.so.6:_dl_mcount_wrapper@GLIBC_2.2.5` |
|   6.3%    751.2 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   5.7%    674.8 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   3.7%    440.4 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   2.7%    324.1 | `<recached_server::replication::ReplHub>::fan_out::{closure#0}` |
|   2.6%    304.9 | `do_rallocx` |
|   2.5%    294.8 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   2.3%    271.0 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   2.3%    269.6 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   2.0%    244.5 | `recached_server::run::{closure#0}::{closure#29}::{closure#0}` |
|   2.0%    236.2 | `<core_engine::request::RequestDecoder>::decode_any` |
|   1.8%    219.3 | `alloc::fmt::format::format_inner` |
|   1.8%    209.2 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   1.7%    208.7 | `_rjem_je_arena_ralloc` |
|   1.7%    200.5 | `[vdso]:+a8c` |
|   1.6%    190.4 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   1.5%    174.4 | `_rjem_je_arena_ralloc_no_move` |
|   1.4%    173.0 | `recached_server::clients::record_metrics` |
|   1.4%    163.9 | `<recached_server::persistence::DeferredAof>::pop` |
|   1.2%    141.9 | `_rjem_malloc` |
|   1.2%    137.8 | `recached_server::propagation::resp_push` |
|   1.1%    136.0 | `<tokio::io::split::ReadHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_read::AsyncRead>::poll_re…` |
|   1.1%    132.8 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   1.0%    124.5 | `core::fmt::write` |
|   1.0%    123.1 | `<tokio::sync::notify::NotifiedProject>::poll_notified` |
|   1.0%    122.7 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   1.0%    120.4 | `tokio::runtime::task::waker::wake_by_val` |
|   0.9%    112.2 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|   0.9%    106.7 | `<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_write::AsyncWrite>::poll…` |
|   0.9%    105.7 | `<alloc::string::String as core::fmt::Write>::write_str` |
|   0.9%    103.5 | `libc.so.6:send@@GLIBC_2.2.5` |
|   0.9%    103.0 | `_rjem_sdallocx` |
|   0.8%    101.2 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   0.8%    100.3 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|   0.8%     99.3 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   0.8%     95.2 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   0.8%     94.3 | `tokio::runtime::task::waker::clone_waker` |
|   0.8%     89.7 | `core_engine::request::build_ref` |
|   0.7%     89.3 | `core_engine::store::entry_size` |
|   0.7%     86.5 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%  11942.1 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  94.7%  11313.6 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  92.5%  11043.1 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  90.2%  10772.0 | `recached_server::run::{closure#0}::{closure#29}::{closure#0}` |
|  87.8%  10484.1 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  61.8%   7380.3 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  53.3%   6363.6 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|  30.3%   3622.9 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|  21.4%   2552.1 | `recached_server::clients::execute_and_record_ref` |
|  17.6%   2103.5 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  16.1%   1921.3 | `recached_server::propagation::broadcast_for_at` |
|  15.9%   1895.2 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|  15.8%   1883.8 | `recached_server::propagation::resp_push` |
|  14.0%   1666.8 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|  13.7%   1637.5 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|  12.5%   1491.5 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|  11.5%   1374.3 | `core_engine::store::write_slot::<&str>` |
|  10.6%   1264.8 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   8.3%    987.9 | `libc.so.6:_dl_mcount_wrapper@GLIBC_2.2.5` |
|   8.1%    971.4 | `<alloc::raw_vec::RawVecInner<_>>::reserve::do_reserve_and_handle::<alloc::alloc::Global>` |
|   7.7%    916.5 | `<alloc::raw_vec::RawVecInner>::finish_grow` |
|   6.7%    803.4 | `do_rallocx` |
|   6.5%    777.3 | `alloc::fmt::format::format_inner` |
|   4.3%    514.1 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   4.1%    492.1 | `core::fmt::write` |
|   4.1%    492.1 | `_rjem_je_arena_ralloc` |
|   3.8%    456.4 | `<core_engine::request::RequestDecoder>::decode_any` |
|   3.8%    453.2 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   3.7%    436.3 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   3.3%    389.1 | `<std::sys::pal::unix::time::Timespec>::now` |
|   3.2%    385.9 | `<recached_server::replication::ReplHub>::fan_out::{closure#0}` |
|   3.2%    379.5 | `<tokio::io::split::ReadHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_read::AsyncRead>::poll_re…` |
|   2.9%    351.6 | `libc.so.6:clock_gettime@GLIBC_2.2.5` |
|   2.9%    348.4 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   2.8%    335.1 | `<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_write::AsyncWrite>::poll…` |
|   2.7%    320.0 | `recached_server::clients::record_metrics` |
|   2.2%    261.9 | `<usize as core::fmt::Display>::fmt` |
|   2.1%    246.3 | `<tokio::runtime::time::Driver>::park_internal` |
|   2.0%    243.5 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   1.9%    228.4 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
