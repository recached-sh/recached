samples: 31085, user cycles/op 13148

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  16.5% |    2171 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   4.2% |     550 |
| connection (decode, reply, socket) | tokio mpsc |   1.9% |     248 |
| connection (decode, reply, socket) | tokio io/split |   1.5% |     192 |
| connection (decode, reply, socket) | tokio wake/schedule |   0.8% |     107 |
| connection (decode, reply, socket) | allocator |   0.6% |      81 |
| **connection (decode, reply, socket)** | **all** | ** 25.5%** | **   3350** |
| store execute | code |  13.4% |    1762 |
| store execute | libc (memcpy, syscall wrappers) |   4.3% |     572 |
| store execute | allocator |   0.1% |      16 |
| **store execute** | **all** | ** 17.9%** | **   2350** |
| frame build (broadcast_for_at) | allocator |   7.7% |    1019 |
| frame build (broadcast_for_at) | code |   6.2% |     814 |
| frame build (broadcast_for_at) | libc (memcpy, syscall wrappers) |   1.0% |     134 |
| **frame build (broadcast_for_at)** | **all** | ** 15.0%** | **   1967** |
| tokio scheduler | tokio wake/schedule |   7.0% |     921 |
| tokio scheduler | code |   1.6% |     211 |
| tokio scheduler | tokio io/split |   1.3% |     168 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.1% |      15 |
| tokio scheduler | allocator |   0.0% |       1 |
| **tokio scheduler** | **all** | ** 10.0%** | **   1316** |
| ordered write path (rest) | code |   5.8% |     760 |
| ordered write path (rest) | tokio mpsc |   0.5% |      70 |
| ordered write path (rest) | libc (memcpy, syscall wrappers) |   0.4% |      55 |
| ordered write path (rest) | allocator |   0.1% |      11 |
| **ordered write path (rest)** | **all** | **  6.8%** | **    896** |
| stripe locks | code |   4.6% |     607 |
| stripe locks | tokio mpsc |   1.6% |     216 |
| **stripe locks** | **all** | **  6.3%** | **    823** |
| repl sender: task (recv, flush, ack read) | code |   2.3% |     307 |
| repl sender: task (recv, flush, ack read) | tokio mpsc |   1.3% |     171 |
| repl sender: task (recv, flush, ack read) | tokio io/split |   1.1% |     143 |
| repl sender: task (recv, flush, ack read) | tokio wake/schedule |   0.8% |      99 |
| repl sender: task (recv, flush, ack read) | libc (memcpy, syscall wrappers) |   0.3% |      40 |
| **repl sender: task (recv, flush, ack read)** | **all** | **  5.8%** | **    761** |
| AOF append/lead/gate | code |   2.4% |     319 |
| AOF append/lead/gate | tokio io/split |   2.0% |     269 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   1.0% |     126 |
| **AOF append/lead/gate** | **all** | **  5.4%** | **    714** |
| repl fan_out: backlog, offset, lock | code |   2.6% |     346 |
| repl fan_out: backlog, offset, lock | atomics/Arc drop |   0.2% |      22 |
| repl fan_out: backlog, offset, lock | allocator |   0.2% |      22 |
| repl fan_out: backlog, offset, lock | libc (memcpy, syscall wrappers) |   0.0% |       1 |
| **repl fan_out: backlog, offset, lock** | **all** | **  3.0%** | **    391** |
| repl fan_out: per-replica enqueue | code |   1.9% |     247 |
| repl fan_out: per-replica enqueue | tokio mpsc |   0.1% |      17 |
| **repl fan_out: per-replica enqueue** | **all** | **  2.0%** | **    264** |
| repl sender: write_repl_frame | libc (memcpy, syscall wrappers) |   0.8% |     100 |
| repl sender: write_repl_frame | tokio io/split |   0.7% |      96 |
| repl sender: write_repl_frame | code |   0.0% |       0 |
| **repl sender: write_repl_frame** | **all** | **  1.5%** | **    196** |
| on_write_logged (frame copy, dispatch) | code |   0.4% |      47 |
| on_write_logged (frame copy, dispatch) | libc (memcpy, syscall wrappers) |   0.1% |      19 |
| on_write_logged (frame copy, dispatch) | allocator |   0.1% |      15 |
| **on_write_logged (frame copy, dispatch)** | **all** | **  0.6%** | **     81** |
| tokio io driver | code |   0.2% |      33 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.0% |       5 |
| **tokio io driver** | **all** | **  0.3%** | **     38** |
| other | code |   0.0% |       0 |
| **other** | **all** | **  0.0%** | **      0** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|   7.7%   1018.1 | `libc.so.6:_dl_mcount_wrapper@GLIBC_2.2.5` |
|   5.3%    690.3 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   4.8%    633.2 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   2.6%    346.4 | `<recached_server::replication::ReplHub>::fan_out::{closure#0}` |
|   2.4%    318.1 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   2.4%    317.2 | `do_rallocx` |
|   2.1%    273.2 | `<core_engine::request::RequestDecoder>::decode_any` |
|   1.9%    247.4 | `<alloc::vec::Vec<recached_server::replication::ReplicaHandle>>::retain::<<recached_server::replication::ReplH…` |
|   1.9%    244.5 | `recached_server::clients::record_metrics` |
|   1.8%    233.0 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   1.6%    215.3 | `_rjem_je_arena_ralloc_no_move` |
|   1.6%    213.6 | `alloc::fmt::format::format_inner` |
|   1.6%    208.5 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   1.5%    194.6 | `recached_server::replication::handle_replica::<tokio::net::tcp::stream::TcpStream>::{closure#0}` |
|   1.4%    189.9 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   1.4%    185.7 | `<recached_server::persistence::DeferredAof>::pop` |
|   1.4%    182.3 | `<tokio::io::split::ReadHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_read::AsyncRead>::poll_re…` |
|   1.4%    181.0 | `<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_write::AsyncWrite>::poll…` |
|   1.4%    179.8 | `_rjem_je_arena_ralloc` |
|   1.3%    172.1 | `recached_server::run::{closure#0}::{closure#29}::{closure#0}` |
|   1.3%    167.9 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   1.3%    165.8 | `[vdso]:+a8c` |
|   1.3%    165.0 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   1.2%    159.9 | `core_engine::request::build_ref` |
|   1.2%    158.2 | `core::str::converts::from_utf8` |
|   1.2%    152.3 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   1.1%    139.2 | `<tokio::sync::batch_semaphore::Acquire as core::future::future::Future>::poll` |
|   1.0%    137.9 | `<tokio::sync::batch_semaphore::Semaphore>::add_permits_locked` |
|   1.0%    130.7 | `recached_server::propagation::try_execute_borrowed` |
|   1.0%    130.7 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   1.0%    129.8 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   1.0%    129.0 | `_rjem_malloc` |
|   1.0%    127.7 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|   1.0%    126.9 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|   1.0%    125.6 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   0.9%    122.7 | `libc.so.6:send@@GLIBC_2.2.5` |
|   0.9%    118.0 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   0.9%    114.6 | `core_engine::store::entry_size` |
|   0.9%    112.5 | `core::fmt::write` |
|   0.8%    111.2 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%  13147.2 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  96.3%  12665.0 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  87.0%  11435.9 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  85.7%  11270.5 | `recached_server::run::{closure#0}::{closure#29}::{closure#0}` |
|  84.2%  11067.9 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  55.0%   7235.1 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  45.5%   5982.7 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|  22.5%   2964.5 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|  21.2%   2789.0 | `recached_server::clients::execute_and_record_ref` |
|  16.9%   2223.9 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  15.0%   1966.8 | `recached_server::propagation::broadcast_for_at` |
|  14.6%   1918.5 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|  14.5%   1912.2 | `recached_server::propagation::resp_push` |
|  12.4%   1636.8 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|  11.8%   1554.4 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|   9.7%   1276.5 | `core_engine::store::write_slot::<&str>` |
|   8.6%   1127.6 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   7.8%   1019.7 | `tokio::runtime::task::raw::poll::<recached_server::replication::run_repl_server::{closure#0}::{closure#0}, al…` |
|   7.7%   1018.1 | `libc.so.6:_dl_mcount_wrapper@GLIBC_2.2.5` |
|   7.4%    967.7 | `<alloc::raw_vec::RawVecInner<_>>::reserve::do_reserve_and_handle::<alloc::alloc::Global>` |
|   7.2%    943.6 | `recached_server::replication::run_repl_server::{closure#0}::{closure#0}` |
|   6.9%    912.7 | `<alloc::raw_vec::RawVecInner>::finish_grow` |
|   6.8%    893.7 | `recached_server::replication::handle_replica::<tokio::net::tcp::stream::TcpStream>::{closure#0}` |
|   6.1%    808.3 | `do_rallocx` |
|   6.1%    796.9 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   5.8%    761.3 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   5.7%    746.1 | `alloc::fmt::format::format_inner` |
|   5.2%    683.1 | `<recached_server::replication::ReplHub>::fan_out::{closure#0}` |
|   5.2%    680.5 | `<core_engine::request::RequestDecoder>::decode_any` |
|   4.2%    557.5 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   4.1%    541.8 | `<tokio::io::split::ReadHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_read::AsyncRead>::poll_re…` |
|   3.7%    485.1 | `_rjem_je_arena_ralloc` |
|   3.6%    479.6 | `<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_write::AsyncWrite>::poll…` |
|   3.6%    471.6 | `core::fmt::write` |
|   3.4%    444.5 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   3.4%    442.0 | `<std::sys::pal::unix::time::Timespec>::now` |
|   3.0%    391.7 | `recached_server::clients::record_metrics` |
|   2.9%    378.5 | `libc.so.6:clock_gettime@GLIBC_2.2.5` |
|   2.7%    359.5 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   2.7%    354.9 | `core_engine::request::build_ref` |
