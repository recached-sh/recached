samples: 25137, user cycles/op 10226

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|   4.5%    458.5 | `frame build (broadcast_for_at, all)` |
|   0.7%     74.4 | `frame build: allocator` |
|   0.0%      0.0 | `frame build: fmt` |
|   0.9%     93.2 | `owned command (to_command)` |
|   3.5%    361.7 | `stripe locks` |
|   5.1%    519.1 | `on_write_logged (all)` |
|   0.0%      0.0 | `append_client_write (queue + ticket)` |
|  12.3%   1253.0 | `gate poll_settled (all)` |
|   2.0%    209.1 | `AOF lead/perform (all)` |
|   0.0%      0.0 | `AOF write(2) wrapper` |
|   0.0%      0.0 | `yield_now` |
|   3.5%    362.9 | `clock reads` |
|   0.0%      0.0 | `now_unix_ms` |
|   2.6%    266.9 | `metrics record` |
|  20.9%   2135.0 | `store execute (all)` |
|   0.3%     26.4 | `reply serialize` |
|   6.2%    638.7 | `decode` |
|   3.3%    338.5 | `allocator (all)` |
|   0.0%      0.0 | `socket send/recv wrappers` |
|  16.4%   1677.8 | `BufWriter flush` |
|   0.0%      0.0 | `Value drop` |
|   0.0%      0.0 | `Command drop` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  20.3% |    2074 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   2.9% |     293 |
| connection (decode, reply, socket) | tokio mpsc |   2.7% |     277 |
| connection (decode, reply, socket) | tokio wake/schedule |   1.1% |     114 |
| connection (decode, reply, socket) | allocator |   0.7% |      76 |
| **connection (decode, reply, socket)** | **all** | ** 27.7%** | **   2835** |
| AOF append/lead/gate | tokio io/split |   8.6% |     882 |
| AOF append/lead/gate | code |   5.5% |     560 |
| AOF append/lead/gate | tokio wake/schedule |   2.6% |     269 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   1.8% |     187 |
| AOF append/lead/gate | allocator |   0.4% |      42 |
| AOF append/lead/gate | atomics/Arc drop |   0.1% |       9 |
| **AOF append/lead/gate** | **all** | ** 19.1%** | **   1948** |
| store execute | code |  13.0% |    1330 |
| store execute | libc (memcpy, syscall wrappers) |   4.7% |     476 |
| store execute | allocator |   0.3% |      31 |
| **store execute** | **all** | ** 18.0%** | **   1836** |
| tokio scheduler | tokio wake/schedule |  11.8% |    1204 |
| tokio scheduler | code |   2.7% |     280 |
| tokio scheduler | tokio io/split |   1.4% |     138 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.2% |      24 |
| tokio scheduler | tokio mpsc |   0.0% |       0 |
| tokio scheduler | allocator |   0.0% |       0 |
| **tokio scheduler** | **all** | ** 16.1%** | **   1648** |
| ordered write path (rest) | code |   4.3% |     438 |
| ordered write path (rest) | tokio mpsc |   0.6% |      63 |
| ordered write path (rest) | libc (memcpy, syscall wrappers) |   0.5% |      54 |
| ordered write path (rest) | allocator |   0.1% |      10 |
| **ordered write path (rest)** | **all** | **  5.5%** | **    565** |
| on_write_logged (frame copy, dispatch) | code |   3.4% |     351 |
| on_write_logged (frame copy, dispatch) | allocator |   1.3% |     132 |
| on_write_logged (frame copy, dispatch) | libc (memcpy, syscall wrappers) |   0.4% |      36 |
| **on_write_logged (frame copy, dispatch)** | **all** | **  5.1%** | **    519** |
| frame build (broadcast_for_at) | code |   3.0% |     310 |
| frame build (broadcast_for_at) | allocator |   0.7% |      74 |
| frame build (broadcast_for_at) | libc (memcpy, syscall wrappers) |   0.7% |      74 |
| **frame build (broadcast_for_at)** | **all** | **  4.5%** | **    458** |
| stripe locks | code |   2.9% |     292 |
| stripe locks | tokio mpsc |   0.7% |      70 |
| **stripe locks** | **all** | **  3.5%** | **    362** |
| tokio io driver | code |   0.4% |      46 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.1% |       9 |
| **tokio io driver** | **all** | **  0.5%** | **     55** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|   8.9%    910.1 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   6.0%    609.8 | `<core_engine::store::CompactHash>::insert::<&str, &[u8]>` |
|   3.4%    347.8 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   3.3%    336.9 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   3.2%    322.2 | `libc.so.6:+1c7919` |
|   3.0%    303.5 | `<core_engine::request::RequestDecoder>::decode_any` |
|   2.6%    266.9 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   2.3%    238.0 | `<recached_server::persistence::DeferredAof>::pop` |
|   2.3%    233.1 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio_rustls::server::TlsStream<tok…` |
|   1.9%    198.9 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   1.9%    190.0 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   1.8%    186.3 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   1.8%    180.6 | `[vdso]:+a8c` |
|   1.6%    166.4 | `<tokio::sync::notify::NotifiedProject>::poll_notified` |
|   1.6%    158.7 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.5%    158.3 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   1.5%    157.4 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   1.5%    151.3 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.5%    150.9 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   1.4%    139.9 | `recached_server::clients::record_metrics` |
|   1.4%    138.3 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   1.2%    120.8 | `core_engine::resp::write_header` |
|   1.2%    118.0 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.1%    117.6 | `_rjem_malloc` |
|   1.1%    117.6 | `recached_server::propagation::resp_push` |
|   1.1%    114.7 | `core_engine::request::build_ref` |
|   1.1%    114.3 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   1.1%    110.3 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   1.0%    105.0 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   1.0%    103.7 | `tokio::runtime::task::waker::wake_by_val` |
|   0.9%     96.8 | `_rjem_sdallocx` |
|   0.9%     95.6 | `<tokio::sync::notify::NotifiedProject>::drop_notified` |
|   0.9%     95.2 | `tokio::runtime::task::waker::clone_waker` |
|   0.8%     84.2 | `<recached_server::persistence::AofShared>::lead` |
|   0.8%     82.2 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   0.8%     79.7 | `recached_server::propagation::try_execute_borrowed` |
|   0.8%     78.5 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   0.8%     78.1 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   0.8%     77.3 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|   0.7%     71.6 | `core::str::converts::from_utf8` |
|   0.7%     67.9 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   0.6%     65.9 | `<core_engine::request::Args>::key_ref` |
|   0.6%     65.1 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   0.6%     64.3 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|   0.6%     63.5 | `<tokio::sync::batch_semaphore::Semaphore>::add_permits_locked` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%  10226.4 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  94.0%   9612.1 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  91.4%   9343.6 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  88.1%   9006.8 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  86.1%   8800.1 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  39.4%   4034.1 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  33.1%   3384.0 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|  20.9%   2135.0 | `recached_server::clients::execute_and_record_ref` |
|  19.4%   1987.8 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|  17.1%   1744.9 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  16.4%   1677.8 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|  15.4%   1574.8 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|  14.9%   1526.8 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|  14.8%   1517.1 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|  12.3%   1253.0 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|  11.2%   1143.6 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|  10.3%   1053.7 | `<core_engine::store::CompactHash>::insert::<&str, &[u8]>` |
|   8.0%    818.1 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio_rustls::server::TlsStream<tok…` |
|   6.2%    638.7 | `<core_engine::request::RequestDecoder>::decode_any` |
|   5.1%    517.9 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   4.6%    469.5 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   4.5%    458.5 | `recached_server::propagation::broadcast_for_at` |
|   3.7%    374.7 | `recached_server::propagation::resp_push` |
|   3.5%    362.9 | `<std::sys::pal::unix::time::Timespec>::now` |
|   3.5%    358.0 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   3.3%    338.1 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   3.2%    322.2 | `libc.so.6:+1c7919` |
|   3.0%    303.5 | `recached_server::persistence::gate_wait::{closure#0}` |
|   2.9%    299.8 | `libc.so.6:+139b5b` |
|   2.9%    299.0 | `core_engine::request::build_ref` |
|   2.8%    289.3 | `<recached_server::persistence::AofDone>::wait::{closure#0}` |
|   2.6%    267.7 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   2.6%    266.9 | `recached_server::clients::record_metrics` |
|   2.5%    252.2 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   2.5%    251.8 | `core_engine::store::write_slot::<&str>` |
|   2.4%    244.5 | `<tokio::runtime::time::Driver>::park_internal` |
|   2.3%    238.0 | `<recached_server::persistence::DeferredAof>::pop` |
|   2.1%    218.9 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   2.0%    209.1 | `<recached_server::persistence::AofShared>::lead` |
|   2.0%    207.1 | `core_engine::resp::write_header` |
|   2.0%    205.4 | `<tokio::runtime::io::driver::Driver>::turn` |
|   1.9%    198.9 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   1.9%    191.6 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   1.9%    190.0 | `<std::thread::lifecycle::spawn_unchecked<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::{closure#0}…` |
|   1.9%    190.0 | `<std::sys::thread::unix::Thread>::new::thread_start` |
