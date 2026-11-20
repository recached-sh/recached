samples: 31467, user cycles/op 13683

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  13.0% |    1780 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   2.4% |     335 |
| connection (decode, reply, socket) | tokio mpsc |   2.2% |     296 |
| connection (decode, reply, socket) | tokio io/split |   1.2% |     167 |
| connection (decode, reply, socket) | tokio wake/schedule |   0.8% |     112 |
| connection (decode, reply, socket) | allocator |   0.4% |      56 |
| **connection (decode, reply, socket)** | **all** | ** 20.1%** | **   2746** |
| store execute | code |  11.1% |    1518 |
| store execute | libc (memcpy, syscall wrappers) |   4.7% |     643 |
| store execute | allocator |   0.1% |      16 |
| **store execute** | **all** | ** 15.9%** | **   2177** |
| tokio scheduler | tokio wake/schedule |   9.3% |    1269 |
| tokio scheduler | code |   3.8% |     525 |
| tokio scheduler | tokio io/split |   1.2% |     168 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.3% |      36 |
| tokio scheduler | allocator |   0.0% |       0 |
| **tokio scheduler** | **all** | ** 14.6%** | **   1999** |
| frame build (broadcast_for_at) | allocator |   6.7% |     915 |
| frame build (broadcast_for_at) | code |   6.1% |     833 |
| frame build (broadcast_for_at) | libc (memcpy, syscall wrappers) |   1.1% |     146 |
| **frame build (broadcast_for_at)** | **all** | ** 13.8%** | **   1894** |
| AOF append/lead/gate | code |   3.3% |     449 |
| AOF append/lead/gate | tokio io/split |   1.8% |     250 |
| AOF append/lead/gate | tokio wake/schedule |   1.5% |     200 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   1.2% |     159 |
| AOF append/lead/gate | allocator |   0.1% |      15 |
| AOF append/lead/gate | atomics/Arc drop |   0.0% |       3 |
| **AOF append/lead/gate** | **all** | **  7.9%** | **   1076** |
| ordered write path (rest) | code |   5.1% |     696 |
| ordered write path (rest) | tokio mpsc |   0.5% |      73 |
| ordered write path (rest) | libc (memcpy, syscall wrappers) |   0.4% |      60 |
| ordered write path (rest) | allocator |   0.1% |      11 |
| **ordered write path (rest)** | **all** | **  6.1%** | **    840** |
| on_write_logged (frame copy, dispatch) | code |   3.6% |     489 |
| on_write_logged (frame copy, dispatch) | libc (memcpy, syscall wrappers) |   1.2% |     165 |
| on_write_logged (frame copy, dispatch) | allocator |   0.8% |     115 |
| **on_write_logged (frame copy, dispatch)** | **all** | **  5.6%** | **    769** |
| repl sender: task (recv, flush, ack read) | code |   2.0% |     270 |
| repl sender: task (recv, flush, ack read) | tokio mpsc |   1.5% |     205 |
| repl sender: task (recv, flush, ack read) | tokio io/split |   1.0% |     132 |
| repl sender: task (recv, flush, ack read) | tokio wake/schedule |   0.7% |     102 |
| repl sender: task (recv, flush, ack read) | libc (memcpy, syscall wrappers) |   0.4% |      55 |
| **repl sender: task (recv, flush, ack read)** | **all** | **  5.6%** | **    764** |
| stripe locks | code |   3.1% |     418 |
| stripe locks | tokio mpsc |   1.2% |     160 |
| **stripe locks** | **all** | **  4.2%** | **    579** |
| repl fan_out: backlog, offset, lock | code |   1.9% |     260 |
| repl fan_out: backlog, offset, lock | allocator |   0.4% |      51 |
| repl fan_out: backlog, offset, lock | atomics/Arc drop |   0.3% |      47 |
| repl fan_out: backlog, offset, lock | libc (memcpy, syscall wrappers) |   0.0% |       2 |
| **repl fan_out: backlog, offset, lock** | **all** | **  2.6%** | **    360** |
| repl fan_out: per-replica enqueue | code |   1.6% |     216 |
| repl fan_out: per-replica enqueue | tokio mpsc |   0.1% |      16 |
| **repl fan_out: per-replica enqueue** | **all** | **  1.7%** | **    232** |
| repl sender: write_repl_frame | tokio io/split |   0.7% |      97 |
| repl sender: write_repl_frame | libc (memcpy, syscall wrappers) |   0.6% |      86 |
| **repl sender: write_repl_frame** | **all** | **  1.3%** | **    183** |
| tokio io driver | code |   0.4% |      53 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.1% |      12 |
| **tokio io driver** | **all** | **  0.5%** | **     65** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|   7.7%   1059.3 | `libc.so.6:_dl_mcount_wrapper@GLIBC_2.2.5` |
|   5.5%    752.7 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   5.3%    721.0 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   3.5%    483.1 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   2.9%    391.8 | `recached_server::run::{closure#0}::{closure#29}::{closure#0}` |
|   2.2%    301.8 | `do_rallocx` |
|   2.0%    279.2 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   2.0%    270.5 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   1.9%    258.3 | `<recached_server::replication::ReplHub>::fan_out::{closure#0}` |
|   1.9%    255.3 | `<core_engine::request::RequestDecoder>::decode_any` |
|   1.6%    218.7 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   1.6%    217.4 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   1.6%    217.0 | `alloc::fmt::format::format_inner` |
|   1.6%    215.7 | `<alloc::vec::Vec<recached_server::replication::ReplicaHandle>>::retain::<<recached_server::replication::ReplH…` |
|   1.5%    208.7 | `_rjem_je_arena_ralloc` |
|   1.5%    202.2 | `[vdso]:+a8c` |
|   1.4%    191.3 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   1.4%    186.5 | `_rjem_je_arena_ralloc_no_move` |
|   1.3%    184.4 | `recached_server::replication::handle_replica::<tokio::net::tcp::stream::TcpStream>::{closure#0}` |
|   1.3%    182.2 | `<recached_server::persistence::DeferredAof>::pop` |
|   1.3%    178.3 | `<tokio::io::split::ReadHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_read::AsyncRead>::poll_re…` |
|   1.2%    168.3 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   1.1%    153.9 | `<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_write::AsyncWrite>::poll…` |
|   1.1%    150.5 | `<tokio::sync::batch_semaphore::Semaphore>::add_permits_locked` |
|   1.1%    146.1 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   1.0%    141.8 | `recached_server::clients::record_metrics` |
|   1.0%    140.5 | `tokio::runtime::task::waker::wake_by_val` |
|   1.0%    139.6 | `core::fmt::write` |
|   1.0%    133.9 | `_rjem_sdallocx` |
|   1.0%    133.1 | `recached_server::propagation::resp_push` |
|   1.0%    132.2 | `<tokio::sync::notify::NotifiedProject>::poll_notified` |
|   1.0%    131.3 | `_rjem_malloc` |
|   1.0%    130.0 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   0.9%    122.6 | `core_engine::request::build_ref` |
|   0.9%    117.8 | `libc.so.6:send@@GLIBC_2.2.5` |
|   0.8%    109.1 | `<alloc::string::String as core::fmt::Write>::write_str` |
|   0.8%    105.7 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|   0.8%    105.2 | `<tokio::sync::batch_semaphore::Acquire as core::future::future::Future>::poll` |
|   0.8%    104.4 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   0.8%    104.4 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%  13683.2 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  94.6%  12944.0 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  85.3%  11671.2 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  83.3%  11392.0 | `recached_server::run::{closure#0}::{closure#29}::{closure#0}` |
|  80.0%  10950.6 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  55.8%   7640.2 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  48.4%   6625.3 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|  28.6%   3916.2 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|  18.2%   2491.2 | `recached_server::clients::execute_and_record_ref` |
|  14.9%   2032.5 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  13.8%   1893.7 | `recached_server::propagation::broadcast_for_at` |
|  13.5%   1852.0 | `recached_server::propagation::resp_push` |
|  13.3%   1818.9 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|  13.2%   1812.0 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|  11.6%   1590.2 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|  11.5%   1571.5 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|   9.5%   1301.5 | `core_engine::store::write_slot::<&str>` |
|   8.7%   1193.6 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   7.7%   1059.3 | `libc.so.6:_dl_mcount_wrapper@GLIBC_2.2.5` |
|   7.3%    998.4 | `tokio::runtime::task::raw::poll::<recached_server::replication::run_repl_server::{closure#0}::{closure#0}, al…` |
|   7.0%    958.4 | `<alloc::raw_vec::RawVecInner<_>>::reserve::do_reserve_and_handle::<alloc::alloc::Global>` |
|   6.7%    914.0 | `recached_server::replication::run_repl_server::{closure#0}::{closure#0}` |
|   6.7%    914.0 | `<alloc::raw_vec::RawVecInner>::finish_grow` |
|   6.5%    885.8 | `recached_server::replication::handle_replica::<tokio::net::tcp::stream::TcpStream>::{closure#0}` |
|   6.0%    816.6 | `do_rallocx` |
|   5.6%    766.2 | `alloc::fmt::format::format_inner` |
|   4.5%    615.7 | `<recached_server::replication::ReplHub>::fan_out::{closure#0}` |
|   4.0%    550.1 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   3.9%    528.3 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   3.8%    522.2 | `<tokio::io::split::ReadHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_read::AsyncRead>::poll_re…` |
|   3.7%    504.9 | `_rjem_je_arena_ralloc` |
|   3.7%    504.4 | `<core_engine::request::RequestDecoder>::decode_any` |
|   3.6%    487.9 | `core::fmt::write` |
|   3.5%    483.5 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   3.2%    438.3 | `<std::sys::pal::unix::time::Timespec>::now` |
|   3.2%    434.0 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   3.1%    420.5 | `<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_write::AsyncWrite>::poll…` |
|   3.0%    406.6 | `libc.so.6:clock_gettime@GLIBC_2.2.5` |
|   2.5%    344.0 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   2.3%    312.2 | `<tokio::runtime::time::Driver>::park_internal` |
