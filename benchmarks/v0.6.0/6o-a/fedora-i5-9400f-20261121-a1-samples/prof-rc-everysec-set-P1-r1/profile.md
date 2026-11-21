samples: 25997, user cycles/op 11840

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|  16.2%   1921.5 | `frame build (broadcast_for_at, all)` |
|   8.2%    966.9 | `frame build: allocator` |
|   6.4%    754.2 | `frame build: fmt` |
|   1.3%    153.9 | `owned command (to_command)` |
|   5.8%    680.9 | `stripe locks` |
|   4.3%    504.6 | `on_write_logged (all)` |
|   0.0%      0.0 | `append_client_write (queue + ticket)` |
|   9.6%   1140.0 | `gate poll_settled (all)` |
|   2.0%    237.3 | `AOF lead/perform (all)` |
|   0.0%      0.0 | `AOF write(2) wrapper` |
|   0.0%      0.0 | `yield_now` |
|   3.9%    456.8 | `clock reads` |
|   0.0%      0.0 | `now_unix_ms` |
|   2.4%    288.3 | `metrics record` |
|  20.3%   2399.7 | `store execute (all)` |
|   0.4%     41.4 | `reply serialize` |
|   4.0%    470.0 | `decode` |
|   9.1%   1076.7 | `allocator (all)` |
|   0.0%      0.0 | `socket send/recv wrappers` |
|  12.8%   1514.8 | `BufWriter flush` |
|   0.0%      0.0 | `Value drop` |
|   0.0%      0.0 | `Command drop` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  15.9% |    1887 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   2.9% |     347 |
| connection (decode, reply, socket) | tokio mpsc |   2.2% |     258 |
| connection (decode, reply, socket) | tokio wake/schedule |   0.9% |     105 |
| connection (decode, reply, socket) | allocator |   0.7% |      80 |
| **connection (decode, reply, socket)** | **all** | ** 22.6%** | **   2677** |
| store execute | code |  12.5% |    1485 |
| store execute | libc (memcpy, syscall wrappers) |   4.9% |     575 |
| store execute | allocator |   0.1% |      16 |
| **store execute** | **all** | ** 17.5%** | **   2076** |
| frame build (broadcast_for_at) | allocator |   8.2% |     967 |
| frame build (broadcast_for_at) | code |   6.9% |     813 |
| frame build (broadcast_for_at) | libc (memcpy, syscall wrappers) |   1.2% |     142 |
| **frame build (broadcast_for_at)** | **all** | ** 16.2%** | **   1921** |
| tokio scheduler | tokio wake/schedule |   9.2% |    1094 |
| tokio scheduler | code |   3.1% |     363 |
| tokio scheduler | tokio io/split |   1.3% |     158 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.3% |      30 |
| tokio scheduler | tokio mpsc |   0.0% |       0 |
| tokio scheduler | allocator |   0.0% |       0 |
| **tokio scheduler** | **all** | ** 13.9%** | **   1646** |
| AOF append/lead/gate | tokio io/split |   5.7% |     669 |
| AOF append/lead/gate | code |   4.2% |     500 |
| AOF append/lead/gate | tokio wake/schedule |   1.8% |     214 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   1.6% |     186 |
| AOF append/lead/gate | allocator |   0.3% |      41 |
| AOF append/lead/gate | atomics/Arc drop |   0.1% |       9 |
| **AOF append/lead/gate** | **all** | ** 13.7%** | **   1618** |
| stripe locks | code |   4.0% |     471 |
| stripe locks | tokio mpsc |   1.8% |     210 |
| **stripe locks** | **all** | **  5.8%** | **    681** |
| ordered write path (rest) | code |   4.4% |     525 |
| ordered write path (rest) | tokio mpsc |   0.6% |      70 |
| ordered write path (rest) | libc (memcpy, syscall wrappers) |   0.5% |      59 |
| ordered write path (rest) | allocator |   0.1% |       9 |
| **ordered write path (rest)** | **all** | **  5.6%** | **    662** |
| on_write_logged (frame copy, dispatch) | code |   3.2% |     375 |
| on_write_logged (frame copy, dispatch) | allocator |   0.8% |      92 |
| on_write_logged (frame copy, dispatch) | libc (memcpy, syscall wrappers) |   0.3% |      38 |
| **on_write_logged (frame copy, dispatch)** | **all** | **  4.3%** | **    505** |
| tokio io driver | code |   0.4% |      42 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.1% |      11 |
| **tokio io driver** | **all** | **  0.5%** | **     53** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|   7.0%    830.7 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   5.6%    658.6 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   3.2%    377.6 | `libc.so.6:+1c7919` |
|   3.1%    369.4 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   2.2%    260.1 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   2.2%    260.1 | `do_rallocx` |
|   2.1%    254.1 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   2.1%    251.4 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   2.0%    241.4 | `_rjem_je_arena_ralloc` |
|   1.9%    219.5 | `<core_engine::request::RequestDecoder>::decode_any` |
|   1.8%    215.4 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio_rustls::server::TlsStream<tok…` |
|   1.8%    209.5 | `[vdso]:+a8c` |
|   1.7%    204.5 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   1.6%    191.7 | `_rjem_je_arena_ralloc_no_move` |
|   1.6%    184.5 | `alloc::fmt::format::format_inner` |
|   1.5%    183.1 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   1.5%    179.0 | `<recached_server::persistence::DeferredAof>::pop` |
|   1.3%    158.5 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   1.3%    157.6 | `_rjem_malloc` |
|   1.3%    156.7 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   1.3%    153.5 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   1.3%    149.8 | `<tokio::sync::batch_semaphore::Acquire as core::future::future::Future>::poll` |
|   1.2%    143.5 | `recached_server::clients::record_metrics` |
|   1.1%    129.8 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.1%    129.3 | `<tokio::sync::notify::NotifiedProject>::poll_notified` |
|   1.1%    125.7 | `core::fmt::write` |
|   1.0%    120.2 | `recached_server::propagation::resp_push` |
|   1.0%    115.7 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.0%    113.4 | `<tokio::sync::mutex::Mutex<std::collections::hash::map::HashMap<alloc::string::String, alloc::vec::Vec<recach…` |
|   0.9%    111.6 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|   0.9%    110.7 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   0.9%    105.2 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   0.9%    104.8 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   0.9%    102.0 | `core_engine::request::build_ref` |
|   0.8%     98.4 | `tokio::runtime::task::waker::wake_by_val` |
|   0.8%     96.6 | `<alloc::string::String as core::fmt::Write>::write_str` |
|   0.8%     94.7 | `_rjem_sdallocx` |
|   0.8%     91.5 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   0.7%     83.3 | `tokio::runtime::task::waker::clone_waker` |
|   0.7%     82.4 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|   0.7%     80.6 | `recached_server::propagation::try_execute_borrowed` |
|   0.7%     80.6 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|   0.7%     79.7 | `<core_engine::request::CommandRef>::to_command` |
|   0.7%     78.3 | `<usize>::_fmt_inner` |
|   0.7%     77.4 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%  11840.0 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  94.9%  11233.4 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  92.7%  10974.7 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  90.6%  10723.3 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  88.2%  10444.1 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  52.1%   6163.9 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  43.7%   5177.9 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|  22.1%   2611.5 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|  20.3%   2399.7 | `recached_server::clients::execute_and_record_ref` |
|  16.3%   1931.1 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  16.2%   1921.5 | `recached_server::propagation::broadcast_for_at` |
|  15.9%   1877.8 | `recached_server::propagation::resp_push` |
|  14.3%   1694.2 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|  14.2%   1684.7 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|  12.8%   1514.8 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|  12.4%   1462.4 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|  12.2%   1440.6 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|  12.2%   1439.2 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|   9.8%   1165.5 | `core_engine::store::write_slot::<&str>` |
|   9.6%   1140.0 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   9.2%   1085.3 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   8.5%   1002.4 | `<alloc::raw_vec::RawVecInner<_>>::reserve::do_reserve_and_handle::<alloc::alloc::Global>` |
|   7.9%    932.7 | `<alloc::raw_vec::RawVecInner>::finish_grow` |
|   6.8%    803.4 | `do_rallocx` |
|   6.7%    794.7 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio_rustls::server::TlsStream<tok…` |
|   6.4%    754.2 | `alloc::fmt::format::format_inner` |
|   5.3%    624.4 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   4.5%    537.0 | `_rjem_je_arena_ralloc` |
|   4.2%    501.0 | `core::fmt::write` |
|   4.2%    498.7 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   4.0%    470.0 | `<core_engine::request::RequestDecoder>::decode_any` |
|   3.9%    456.8 | `<std::sys::pal::unix::time::Timespec>::now` |
|   3.6%    431.3 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   3.2%    377.6 | `libc.so.6:+1c7919` |
|   3.1%    365.3 | `libc.so.6:+139b5b` |
|   2.9%    347.0 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   2.7%    323.4 | `<tokio::sync::mutex::Mutex<std::collections::hash::map::HashMap<alloc::string::String, alloc::vec::Vec<recach…` |
|   2.4%    288.3 | `recached_server::clients::record_metrics` |
|   2.4%    278.7 | `<usize as core::fmt::Display>::fmt` |
|   2.1%    251.9 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   2.1%    250.5 | `recached_server::persistence::gate_wait::{closure#0}` |
|   2.1%    249.1 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   2.1%    245.9 | `<tokio::runtime::time::Driver>::park_internal` |
|   2.0%    237.3 | `<recached_server::persistence::AofShared>::lead` |
|   2.0%    234.1 | `<recached_server::persistence::AofDone>::wait::{closure#0}` |
