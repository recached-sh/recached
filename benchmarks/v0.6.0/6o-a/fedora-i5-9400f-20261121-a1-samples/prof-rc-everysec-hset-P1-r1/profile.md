samples: 27101, user cycles/op 12831

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|  20.1%   2583.1 | `frame build (broadcast_for_at, all)` |
|   9.5%   1223.4 | `frame build: allocator` |
|   7.7%    988.1 | `frame build: fmt` |
|   1.1%    142.0 | `owned command (to_command)` |
|   3.9%    506.1 | `stripe locks` |
|   3.7%    478.6 | `on_write_logged (all)` |
|   0.0%      0.0 | `append_client_write (queue + ticket)` |
|   9.3%   1198.7 | `gate poll_settled (all)` |
|   1.8%    227.7 | `AOF lead/perform (all)` |
|   0.0%      0.0 | `AOF write(2) wrapper` |
|   0.0%      0.0 | `yield_now` |
|   3.4%    440.8 | `clock reads` |
|   0.0%      0.0 | `now_unix_ms` |
|   3.1%    398.6 | `metrics record` |
|  20.1%   2582.1 | `store execute (all)` |
|   0.2%     23.7 | `reply serialize` |
|   5.4%    693.1 | `decode` |
|  10.3%   1320.9 | `allocator (all)` |
|   0.0%      0.0 | `socket send/recv wrappers` |
|  12.5%   1602.6 | `BufWriter flush` |
|   0.0%      0.0 | `Value drop` |
|   0.0%      0.0 | `Command drop` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  17.8% |    2280 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   2.5% |     316 |
| connection (decode, reply, socket) | tokio mpsc |   1.9% |     247 |
| connection (decode, reply, socket) | tokio wake/schedule |   0.8% |     104 |
| connection (decode, reply, socket) | allocator |   0.7% |      88 |
| **connection (decode, reply, socket)** | **all** | ** 23.7%** | **   3035** |
| frame build (broadcast_for_at) | allocator |   9.5% |    1223 |
| frame build (broadcast_for_at) | code |   8.8% |    1131 |
| frame build (broadcast_for_at) | libc (memcpy, syscall wrappers) |   1.8% |     229 |
| **frame build (broadcast_for_at)** | **all** | ** 20.1%** | **   2583** |
| store execute | code |  12.8% |    1638 |
| store execute | libc (memcpy, syscall wrappers) |   3.8% |     489 |
| store execute | allocator |   0.2% |      20 |
| **store execute** | **all** | ** 16.7%** | **   2146** |
| AOF append/lead/gate | tokio io/split |   5.6% |     713 |
| AOF append/lead/gate | code |   4.1% |     523 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   1.8% |     229 |
| AOF append/lead/gate | tokio wake/schedule |   1.6% |     204 |
| AOF append/lead/gate | allocator |   0.3% |      39 |
| AOF append/lead/gate | atomics/Arc drop |   0.1% |      11 |
| **AOF append/lead/gate** | **all** | ** 13.4%** | **   1720** |
| tokio scheduler | tokio wake/schedule |   8.8% |    1127 |
| tokio scheduler | code |   2.8% |     364 |
| tokio scheduler | tokio io/split |   0.9% |     121 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.2% |      31 |
| tokio scheduler | allocator |   0.0% |       1 |
| tokio scheduler | tokio mpsc |   0.0% |       0 |
| **tokio scheduler** | **all** | ** 12.8%** | **   1645** |
| ordered write path (rest) | code |   4.1% |     525 |
| ordered write path (rest) | libc (memcpy, syscall wrappers) |   0.5% |      67 |
| ordered write path (rest) | tokio mpsc |   0.5% |      64 |
| ordered write path (rest) | allocator |   0.1% |      11 |
| **ordered write path (rest)** | **all** | **  5.2%** | **    667** |
| stripe locks | code |   3.2% |     413 |
| stripe locks | tokio mpsc |   0.7% |      93 |
| **stripe locks** | **all** | **  3.9%** | **    506** |
| on_write_logged (frame copy, dispatch) | code |   2.8% |     364 |
| on_write_logged (frame copy, dispatch) | allocator |   0.7% |      84 |
| on_write_logged (frame copy, dispatch) | libc (memcpy, syscall wrappers) |   0.2% |      31 |
| **on_write_logged (frame copy, dispatch)** | **all** | **  3.7%** | **    479** |
| tokio io driver | code |   0.3% |      39 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.1% |      11 |
| **tokio io driver** | **all** | **  0.4%** | **     51** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|   6.7%    864.0 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   5.3%    674.2 | `<core_engine::store::CompactHash>::insert::<&str, &[u8]>` |
|   2.8%    359.3 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   2.6%    330.0 | `libc.so.6:+1c7919` |
|   2.2%    285.0 | `_rjem_je_arena_ralloc` |
|   2.2%    276.0 | `<core_engine::request::RequestDecoder>::decode_any` |
|   2.1%    271.8 | `do_rallocx` |
|   2.1%    268.0 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio_rustls::server::TlsStream<tok…` |
|   2.0%    256.1 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   1.9%    243.3 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   1.9%    241.0 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   1.8%    234.8 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   1.8%    226.8 | `alloc::fmt::format::format_inner` |
|   1.7%    216.8 | `_rjem_malloc` |
|   1.6%    205.9 | `_rjem_sdallocx` |
|   1.6%    205.5 | `core_engine::request::build_ref` |
|   1.6%    201.2 | `_rjem_je_arena_ralloc_no_move` |
|   1.5%    193.6 | `<recached_server::persistence::DeferredAof>::pop` |
|   1.5%    192.2 | `[vdso]:+a8c` |
|   1.4%    185.1 | `recached_server::clients::record_metrics` |
|   1.4%    181.3 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   1.3%    170.0 | `core::fmt::write` |
|   1.2%    159.1 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   1.2%    152.0 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   1.2%    148.2 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.1%    142.0 | `<alloc::string::String as core::fmt::Write>::write_str` |
|   1.0%    134.5 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   1.0%    133.5 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.0%    130.2 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.0%    125.0 | `recached_server::propagation::resp_push` |
|   1.0%    123.6 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   0.9%    121.2 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   0.9%    121.2 | `<tokio::sync::notify::NotifiedProject>::poll_notified` |
|   0.9%    111.7 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   0.9%    109.8 | `tokio::runtime::task::waker::wake_by_val` |
|   0.8%    107.9 | `<usize>::_fmt_inner` |
|   0.8%    104.2 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   0.8%    103.2 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|   0.8%     98.5 | `<tokio::sync::mutex::Mutex<std::collections::hash::map::HashMap<alloc::string::String, alloc::vec::Vec<recach…` |
|   0.7%     94.2 | `<core::fmt::Formatter>::pad_integral` |
|   0.7%     93.3 | `tokio::runtime::task::waker::clone_waker` |
|   0.7%     91.8 | `recached_server::propagation::try_execute_borrowed` |
|   0.7%     86.2 | `<recached_server::replication::ReplHub>::stripe_for` |
|   0.7%     85.2 | `libc.so.6:+175818` |
|   0.7%     83.8 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%  12830.6 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  94.8%  12169.7 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  92.9%  11923.5 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  91.1%  11682.5 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  88.9%  11405.6 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  53.0%   6805.2 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  46.8%   5998.9 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|  25.5%   3274.8 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|  20.1%   2583.1 | `recached_server::propagation::broadcast_for_at` |
|  20.1%   2582.1 | `recached_server::clients::execute_and_record_ref` |
|  15.7%   2011.6 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  15.2%   1944.9 | `recached_server::propagation::resp_push` |
|  13.8%   1766.4 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|  13.6%   1741.3 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|  13.0%   1664.6 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|  12.5%   1602.6 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|  11.9%   1531.6 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   9.3%   1198.7 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   8.8%   1132.5 | `<core_engine::store::CompactHash>::insert::<&str, &[u8]>` |
|   7.8%    998.0 | `<alloc::raw_vec::RawVecInner>::finish_grow` |
|   7.7%    988.1 | `alloc::fmt::format::format_inner` |
|   6.8%    874.4 | `do_rallocx` |
|   6.5%    836.1 | `<alloc::raw_vec::RawVecInner<_>>::reserve::do_reserve_and_handle::<alloc::alloc::Global>` |
|   6.4%    818.1 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio_rustls::server::TlsStream<tok…` |
|   5.4%    693.1 | `<core_engine::request::RequestDecoder>::decode_any` |
|   5.3%    683.2 | `core::fmt::write` |
|   4.6%    595.6 | `_rjem_je_arena_ralloc` |
|   3.7%    475.3 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   3.7%    474.9 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   3.4%    440.8 | `<std::sys::pal::unix::time::Timespec>::now` |
|   3.3%    426.1 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   3.1%    398.6 | `recached_server::clients::record_metrics` |
|   3.0%    383.5 | `<usize as core::fmt::Display>::fmt` |
|   2.9%    377.8 | `core_engine::request::build_ref` |
|   2.8%    360.3 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   2.6%    338.5 | `libc.so.6:+139b5b` |
|   2.6%    330.0 | `libc.so.6:+1c7919` |
|   2.2%    285.5 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   2.1%    270.8 | `core_engine::store::write_slot::<&str>` |
|   2.0%    251.4 | `<tokio::runtime::time::Driver>::park_internal` |
|   1.9%    243.3 | `<alloc::raw_vec::RawVec<std::ffi::os_str::OsString>>::grow_one` |
|   1.9%    241.9 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   1.8%    235.3 | `recached_server::persistence::gate_wait::{closure#0}` |
|   1.8%    234.8 | `libc.so.6:+edc42` |
|   1.8%    234.8 | `libc.so.6:+17319c` |
