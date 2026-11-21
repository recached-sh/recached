samples: 24605, user cycles/op 9993

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|   3.6%    355.4 | `frame build (broadcast_for_at, all)` |
|   0.3%     29.6 | `frame build: allocator` |
|   0.0%      0.0 | `frame build: fmt` |
|   0.7%     69.9 | `owned command (to_command)` |
|   5.1%    512.9 | `stripe locks` |
|   5.5%    553.9 | `on_write_logged (all)` |
|   0.0%      0.0 | `append_client_write (queue + ticket)` |
|  11.1%   1114.0 | `gate poll_settled (all)` |
|   2.1%    205.9 | `AOF lead/perform (all)` |
|   0.0%      0.0 | `AOF write(2) wrapper` |
|   0.0%      0.0 | `yield_now` |
|   4.1%    405.7 | `clock reads` |
|   0.0%      0.0 | `now_unix_ms` |
|   2.6%    259.5 | `metrics record` |
|  24.1%   2408.7 | `store execute (all)` |
|   0.5%     48.7 | `reply serialize` |
|   5.0%    499.1 | `decode` |
|   2.9%    285.9 | `allocator (all)` |
|   0.0%      0.0 | `socket send/recv wrappers` |
|  14.4%   1438.1 | `BufWriter flush` |
|   0.0%      0.0 | `Value drop` |
|   0.0%      0.0 | `Command drop` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  18.1% |    1807 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   3.0% |     301 |
| connection (decode, reply, socket) | tokio mpsc |   2.7% |     265 |
| connection (decode, reply, socket) | tokio wake/schedule |   0.9% |      89 |
| connection (decode, reply, socket) | allocator |   0.5% |      54 |
| **connection (decode, reply, socket)** | **all** | ** 25.2%** | **   2516** |
| store execute | code |  15.2% |    1520 |
| store execute | libc (memcpy, syscall wrappers) |   5.7% |     569 |
| store execute | allocator |   0.2% |      20 |
| **store execute** | **all** | ** 21.1%** | **   2109** |
| AOF append/lead/gate | tokio io/split |   6.6% |     661 |
| AOF append/lead/gate | code |   5.3% |     531 |
| AOF append/lead/gate | tokio wake/schedule |   2.2% |     222 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   1.9% |     190 |
| AOF append/lead/gate | allocator |   0.4% |      41 |
| AOF append/lead/gate | atomics/Arc drop |   0.1% |       6 |
| **AOF append/lead/gate** | **all** | ** 16.5%** | **   1650** |
| tokio scheduler | tokio wake/schedule |  11.3% |    1130 |
| tokio scheduler | code |   3.4% |     337 |
| tokio scheduler | tokio io/split |   1.2% |     117 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.4% |      35 |
| tokio scheduler | allocator |   0.0% |       0 |
| tokio scheduler | tokio mpsc |   0.0% |       0 |
| **tokio scheduler** | **all** | ** 16.2%** | **   1621** |
| ordered write path (rest) | code |   4.9% |     489 |
| ordered write path (rest) | tokio mpsc |   0.7% |      69 |
| ordered write path (rest) | libc (memcpy, syscall wrappers) |   0.6% |      57 |
| ordered write path (rest) | allocator |   0.1% |       9 |
| **ordered write path (rest)** | **all** | **  6.2%** | **    623** |
| on_write_logged (frame copy, dispatch) | code |   3.7% |     372 |
| on_write_logged (frame copy, dispatch) | allocator |   1.5% |     145 |
| on_write_logged (frame copy, dispatch) | libc (memcpy, syscall wrappers) |   0.4% |      37 |
| **on_write_logged (frame copy, dispatch)** | **all** | **  5.5%** | **    554** |
| stripe locks | code |   3.8% |     383 |
| stripe locks | tokio mpsc |   1.3% |     130 |
| **stripe locks** | **all** | **  5.1%** | **    513** |
| frame build (broadcast_for_at) | code |   2.6% |     259 |
| frame build (broadcast_for_at) | libc (memcpy, syscall wrappers) |   0.7% |      67 |
| frame build (broadcast_for_at) | allocator |   0.3% |      30 |
| **frame build (broadcast_for_at)** | **all** | **  3.6%** | **    355** |
| tokio io driver | code |   0.4% |      38 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.1% |      14 |
| **tokio io driver** | **all** | **  0.5%** | **     52** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|   8.1%    811.4 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   6.5%    649.4 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   3.7%    367.9 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   3.1%    311.1 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   2.7%    267.2 | `<core_engine::request::RequestDecoder>::decode_any` |
|   2.5%    248.1 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   2.4%    235.1 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   2.2%    220.1 | `<recached_server::persistence::DeferredAof>::pop` |
|   2.1%    208.3 | `libc.so.6:+1c7919` |
|   1.9%    193.7 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio_rustls::server::TlsStream<tok…` |
|   1.8%    182.8 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   1.8%    181.1 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   1.8%    175.4 | `[vdso]:+a8c` |
|   1.7%    173.8 | `libc.so.6:+1c79c7` |
|   1.7%    164.9 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   1.6%    155.5 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|   1.4%    140.1 | `<tokio::sync::notify::NotifiedProject>::poll_notified` |
|   1.4%    137.7 | `recached_server::clients::record_metrics` |
|   1.2%    124.3 | `<tokio::sync::batch_semaphore::Acquire as core::future::future::Future>::poll` |
|   1.2%    119.4 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   1.2%    117.4 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.2%    117.4 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   1.2%    114.9 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.1%    112.1 | `recached_server::propagation::resp_push` |
|   1.1%    111.7 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.1%    109.2 | `core_engine::request::build_ref` |
|   1.1%    108.4 | `tokio::runtime::task::waker::wake_by_val` |
|   1.1%    107.6 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   1.1%    105.6 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|   1.0%    103.6 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   1.0%    102.7 | `<tokio::sync::mutex::Mutex<std::collections::hash::map::HashMap<alloc::string::String, alloc::vec::Vec<recach…` |
|   0.9%     94.6 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   0.9%     92.6 | `tokio::runtime::task::waker::clone_waker` |
|   0.9%     88.9 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   0.9%     86.1 | `recached_server::propagation::try_execute_borrowed` |
|   0.9%     86.1 | `_rjem_malloc` |
|   0.8%     84.1 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   0.8%     77.2 | `<recached_server::persistence::AofShared>::lead` |
|   0.7%     73.9 | `<tokio::sync::notify::NotifiedProject>::drop_notified` |
|   0.7%     73.1 | `core_engine::store::lock_stripe` |
|   0.7%     72.7 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   0.7%     72.3 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|   0.7%     70.7 | `core::str::converts::from_utf8` |
|   0.7%     68.6 | `<tokio::sync::batch_semaphore::Semaphore>::add_permits_locked` |
|   0.6%     61.7 | `core_engine::store::entry_size` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%   9992.6 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  93.9%   9387.1 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  91.6%   9149.9 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  88.5%   8838.8 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  85.6%   8554.1 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  44.5%   4445.4 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  36.2%   3615.7 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|  24.1%   2408.7 | `recached_server::clients::execute_and_record_ref` |
|  19.9%   1984.7 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  17.8%   1777.2 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|  16.6%   1657.8 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|  15.2%   1518.9 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|  14.9%   1492.5 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|  14.4%   1438.1 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|  13.3%   1330.5 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|  11.8%   1179.0 | `core_engine::store::write_slot::<&str>` |
|  11.1%   1114.0 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|  10.9%   1090.0 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|  10.9%   1086.0 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|   7.2%    720.9 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio_rustls::server::TlsStream<tok…` |
|   5.5%    551.1 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   5.0%    499.1 | `<core_engine::request::RequestDecoder>::decode_any` |
|   4.8%    482.1 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   4.3%    426.8 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   4.1%    405.7 | `<std::sys::pal::unix::time::Timespec>::now` |
|   3.6%    355.4 | `recached_server::propagation::broadcast_for_at` |
|   3.5%    353.3 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   3.3%    328.6 | `libc.so.6:+139b5b` |
|   3.1%    306.2 | `recached_server::propagation::resp_push` |
|   2.6%    259.5 | `recached_server::clients::record_metrics` |
|   2.6%    256.3 | `recached_server::persistence::gate_wait::{closure#0}` |
|   2.5%    246.1 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   2.5%    244.9 | `<tokio::runtime::time::Driver>::park_internal` |
|   2.4%    241.6 | `<recached_server::persistence::AofDone>::wait::{closure#0}` |
|   2.3%    233.1 | `<tokio::sync::mutex::Mutex<std::collections::hash::map::HashMap<alloc::string::String, alloc::vec::Vec<recach…` |
|   2.2%    220.9 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   2.2%    220.1 | `<recached_server::persistence::DeferredAof>::pop` |
|   2.1%    208.3 | `libc.so.6:+1c7919` |
|   2.1%    206.7 | `<tokio::runtime::io::driver::Driver>::turn` |
|   2.1%    205.9 | `<recached_server::persistence::AofShared>::lead` |
|   1.9%    193.7 | `core_engine::request::build_ref` |
|   1.8%    182.8 | `libc.so.6:+17319c` |
|   1.8%    182.8 | `<std::thread::lifecycle::spawn_unchecked<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::{closure#0}…` |
|   1.8%    182.8 | `std::sys::backtrace::__rust_begin_short_backtrace::<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::…` |
|   1.8%    182.8 | `<std::sys::thread::unix::Thread>::new::thread_start` |
