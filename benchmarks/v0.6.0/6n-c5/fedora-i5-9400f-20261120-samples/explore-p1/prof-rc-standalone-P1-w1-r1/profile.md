samples: 25240, user cycles/op 11617

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  15.9% |    1852 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   2.8% |     328 |
| connection (decode, reply, socket) | tokio mpsc |   2.3% |     265 |
| connection (decode, reply, socket) | tokio io/split |   1.6% |     182 |
| connection (decode, reply, socket) | tokio wake/schedule |   0.9% |      99 |
| connection (decode, reply, socket) | allocator |   0.5% |      52 |
| **connection (decode, reply, socket)** | **all** | ** 23.9%** | **   2778** |
| store execute | code |  12.2% |    1412 |
| store execute | libc (memcpy, syscall wrappers) |   4.8% |     563 |
| store execute | allocator |   0.1% |      14 |
| **store execute** | **all** | ** 17.1%** | **   1989** |
| frame build (broadcast_for_at) | allocator |   7.8% |     902 |
| frame build (broadcast_for_at) | code |   7.0% |     815 |
| frame build (broadcast_for_at) | libc (memcpy, syscall wrappers) |   1.3% |     148 |
| **frame build (broadcast_for_at)** | **all** | ** 16.0%** | **   1864** |
| tokio scheduler | tokio wake/schedule |  10.1% |    1179 |
| tokio scheduler | code |   3.3% |     388 |
| tokio scheduler | tokio io/split |   1.1% |     126 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.2% |      23 |
| tokio scheduler | allocator |   0.0% |       0 |
| **tokio scheduler** | **all** | ** 14.8%** | **   1716** |
| AOF append/lead/gate | tokio io/split |   5.3% |     619 |
| AOF append/lead/gate | code |   4.6% |     539 |
| AOF append/lead/gate | tokio wake/schedule |   1.9% |     218 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   1.5% |     169 |
| AOF append/lead/gate | allocator |   0.4% |      44 |
| AOF append/lead/gate | atomics/Arc drop |   0.1% |       8 |
| **AOF append/lead/gate** | **all** | ** 13.7%** | **   1596** |
| ordered write path (rest) | code |   4.3% |     501 |
| ordered write path (rest) | tokio mpsc |   0.5% |      63 |
| ordered write path (rest) | libc (memcpy, syscall wrappers) |   0.5% |      58 |
| ordered write path (rest) | allocator |   0.1% |      10 |
| **ordered write path (rest)** | **all** | **  5.4%** | **    632** |
| stripe locks | code |   3.7% |     425 |
| stripe locks | tokio mpsc |   1.0% |     111 |
| **stripe locks** | **all** | **  4.6%** | **    536** |
| on_write_logged (frame copy, dispatch) | code |   2.8% |     329 |
| on_write_logged (frame copy, dispatch) | allocator |   0.8% |      97 |
| on_write_logged (frame copy, dispatch) | libc (memcpy, syscall wrappers) |   0.2% |      29 |
| **on_write_logged (frame copy, dispatch)** | **all** | **  3.9%** | **    455** |
| tokio io driver | code |   0.4% |      41 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.1% |       9 |
| **tokio io driver** | **all** | **  0.4%** | **     50** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|   6.7%    783.8 | `libc.so.6:_dl_mcount_wrapper@GLIBC_2.2.5` |
|   6.7%    774.2 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   5.4%    632.9 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   2.8%    325.0 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   2.6%    307.5 | `do_rallocx` |
|   2.5%    284.9 | `recached_server::run::{closure#0}::{closure#29}::{closure#0}` |
|   2.4%    278.9 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   2.3%    264.7 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   2.1%    249.5 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio_rustls::server::TlsStream<tok…` |
|   1.9%    225.5 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   1.8%    212.2 | `<core_engine::request::RequestDecoder>::decode_any` |
|   1.8%    203.9 | `alloc::fmt::format::format_inner` |
|   1.7%    200.2 | `_rjem_je_arena_ralloc` |
|   1.7%    191.9 | `<recached_server::persistence::DeferredAof>::pop` |
|   1.6%    185.5 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   1.5%    174.9 | `[vdso]:+a8c` |
|   1.5%    173.1 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   1.4%    162.0 | `_rjem_je_arena_ralloc_no_move` |
|   1.3%    156.5 | `core_engine::request::build_ref` |
|   1.3%    148.2 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   1.2%    136.7 | `recached_server::clients::record_metrics` |
|   1.1%    128.9 | `<tokio::sync::notify::NotifiedProject>::poll_notified` |
|   1.1%    126.6 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.1%    126.1 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   1.1%    125.7 | `<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_write::AsyncWrite>::poll…` |
|   1.1%    124.7 | `core::fmt::write` |
|   1.1%    122.4 | `<tokio::io::split::ReadHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_read::AsyncRead>::poll_re…` |
|   1.0%    115.5 | `_rjem_malloc` |
|   1.0%    115.1 | `recached_server::propagation::resp_push` |
|   0.9%    107.7 | `core::str::converts::from_utf8` |
|   0.9%    107.2 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.9%    105.9 | `_rjem_sdallocx` |
|   0.9%    104.5 | `tokio::runtime::task::waker::wake_by_val` |
|   0.9%    104.0 | `libc.so.6:send@@GLIBC_2.2.5` |
|   0.9%    102.6 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   0.9%    101.3 | `<tokio::sync::mutex::Mutex<std::collections::hash::map::HashMap<alloc::string::String, alloc::vec::Vec<recach…` |
|   0.9%     99.4 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   0.9%     99.0 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   0.8%     97.1 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|   0.8%     92.1 | `<alloc::string::String as core::fmt::Write>::write_str` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%  11617.3 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  94.5%  10977.5 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  92.1%  10697.6 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  89.8%  10433.0 | `recached_server::run::{closure#0}::{closure#29}::{closure#0}` |
|  87.1%  10120.0 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  49.6%   5760.3 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  42.5%   4933.2 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|  21.4%   2486.4 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|  19.9%   2314.2 | `recached_server::clients::execute_and_record_ref` |
|  16.1%   1865.9 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  16.0%   1864.1 | `recached_server::propagation::broadcast_for_at` |
|  15.8%   1833.7 | `recached_server::propagation::resp_push` |
|  14.2%   1648.7 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|  13.3%   1541.4 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|  12.5%   1454.5 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|  12.2%   1419.5 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|  10.0%   1165.9 | `core_engine::store::write_slot::<&str>` |
|   9.9%   1154.8 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   9.3%   1077.5 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   8.2%    956.9 | `<alloc::raw_vec::RawVecInner<_>>::reserve::do_reserve_and_handle::<alloc::alloc::Global>` |
|   7.7%    897.5 | `<alloc::raw_vec::RawVecInner>::finish_grow` |
|   6.9%    800.9 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio_rustls::server::TlsStream<tok…` |
|   6.8%    794.4 | `do_rallocx` |
|   6.7%    783.8 | `libc.so.6:_dl_mcount_wrapper@GLIBC_2.2.5` |
|   6.7%    777.4 | `alloc::fmt::format::format_inner` |
|   4.5%    524.2 | `<core_engine::request::RequestDecoder>::decode_any` |
|   4.4%    515.0 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   4.4%    508.6 | `core::fmt::write` |
|   4.1%    481.0 | `_rjem_je_arena_ralloc` |
|   3.9%    452.4 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   3.7%    435.4 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   3.4%    398.6 | `<std::sys::pal::unix::time::Timespec>::now` |
|   3.3%    384.3 | `<tokio::io::split::ReadHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_read::AsyncRead>::poll_re…` |
|   3.2%    368.7 | `<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_write::AsyncWrite>::poll…` |
|   3.1%    360.4 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   3.0%    352.1 | `libc.so.6:clock_gettime@GLIBC_2.2.5` |
|   2.5%    285.8 | `<usize as core::fmt::Display>::fmt` |
|   2.4%    281.2 | `recached_server::clients::record_metrics` |
|   2.4%    279.4 | `core_engine::request::build_ref` |
|   2.3%    261.9 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
