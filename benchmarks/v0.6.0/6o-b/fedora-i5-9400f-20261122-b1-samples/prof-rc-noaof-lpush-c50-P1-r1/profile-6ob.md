samples: 22536, user cycles/op 8676

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|  20.9%   1812.8 | `decode (all)` |
|  14.0%   1216.9 | `decode: general Value path (Args::general)` |
|  11.2%    968.2 | `decode: from_value` |
|   0.0%      0.0 | `decode: build (owned MSET/DEL)` |
|   0.0%      0.0 | `to_command (borrowed -> owned)` |
|   0.0%      0.0 | `borrowed execute (try_execute_borrowed)` |
|  35.3%   3061.3 | `ordered write entry (execute_ordered_write)` |
|  33.0%   2863.4 | `store execute (all)` |
|   4.0%    348.0 | `store: Command clone` |
|   2.7%    233.3 | `store: key hash (sip)` |
|   4.0%    346.1 | `store: entry_state` |
|   2.2%    191.7 | `store: extract_key` |
|   3.8%    325.7 | `clock reads` |
|   3.7%    321.1 | `metrics record` |
|   0.4%     35.8 | `reply serialize (Value)` |
|   0.8%     70.8 | `Value drop` |
|   0.5%     46.6 | `Command drop` |
|   5.8%    499.7 | `allocator (all)` |
|   5.0%    436.2 | `select overflow arm (mpsc recv)` |
|   7.9%    685.6 | `BufWriter flush (all)` |
|   9.3%    806.1 | `AofGatedWriter (all)` |
|   1.8%    157.8 | `DeferredAof pop` |
|   3.1%    270.6 | `socket read (poll_read_priv)` |
|   2.8%    243.7 | `socket write (poll_write_priv)` |
|   9.3%    806.1 | `split lock (ReadHalf/WriteHalf)` |
|   2.5%    216.0 | `tokio park/driver` |
|  95.8%   8314.2 | `tokio run_task (scheduler)` |
|   0.0%      0.0 | `task-local scope (with_deferred_aof)` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  34.2% |    2965 |
| connection (decode, reply, socket) | allocator |   4.3% |     376 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   3.9% |     341 |
| connection (decode, reply, socket) | tokio mpsc |   2.8% |     245 |
| connection (decode, reply, socket) | tokio wake/schedule |   1.2% |     103 |
| **connection (decode, reply, socket)** | **all** | ** 46.5%** | **   4030** |
| store execute | code |  22.9% |    1989 |
| store execute | libc (memcpy, syscall wrappers) |   4.1% |     352 |
| store execute | allocator |   1.4% |     123 |
| **store execute** | **all** | ** 28.4%** | **   2464** |
| tokio scheduler | tokio wake/schedule |   9.1% |     793 |
| tokio scheduler | code |   1.7% |     147 |
| tokio scheduler | tokio io/split |   1.5% |     128 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.2% |      20 |
| tokio scheduler | allocator |   0.0% |       1 |
| tokio scheduler | tokio mpsc |   0.0% |       0 |
| **tokio scheduler** | **all** | ** 12.6%** | **   1089** |
| AOF append/lead/gate | tokio io/split |   4.5% |     386 |
| AOF append/lead/gate | code |   3.5% |     307 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   1.8% |     153 |
| **AOF append/lead/gate** | **all** | **  9.7%** | **    846** |
| ordered write path (rest) | code |   2.3% |     198 |
| **ordered write path (rest)** | **all** | **  2.3%** | **    198** |
| tokio io driver | code |   0.4% |      37 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.1% |      12 |
| **tokio io driver** | **all** | **  0.6%** | **     49** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|  10.4%    903.5 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   5.4%    465.8 | `<core_engine::request::RequestDecoder>::decode_any` |
|   4.5%    392.3 | `<core_engine::cmd::Command>::from_value` |
|   3.7%    323.8 | `_rjem_malloc` |
|   3.6%    315.3 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   2.6%    225.6 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   2.3%    203.6 | `<core_engine::store::KeyValueStore>::execute_inner` |
|   2.2%    194.0 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   2.2%    190.2 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   2.1%    184.0 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   2.0%    173.2 | `recached_server::clients::record_metrics` |
|   1.8%    157.8 | `<recached_server::persistence::DeferredAof>::pop` |
|   1.7%    149.4 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   1.5%    128.2 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   1.5%    128.2 | `_rjem_sdallocx` |
|   1.4%    124.0 | `libc.so.6:+1c7d91` |
|   1.4%    119.7 | `<core_engine::cmd::Command as core::clone::Clone>::clone` |
|   1.4%    118.2 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   1.3%    115.1 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.3%    113.6 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   1.3%    109.3 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   1.2%    105.1 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.2%    103.2 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   1.2%    102.4 | `[vdso]:+a8c` |
|   1.2%    102.0 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   1.1%     95.5 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   1.1%     94.3 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   1.0%     88.2 | `core_engine::store::lock_stripe` |
|   0.9%     81.2 | `core_engine::store::write_slot::<alloc::string::String>` |
|   0.9%     78.5 | `recached_server::clients::command_name` |
|   0.9%     77.4 | `tokio::runtime::task::waker::clone_waker` |
|   0.8%     70.8 | `core::ptr::drop_glue::<core_engine::resp::Value>` |
|   0.8%     70.1 | `core_engine::store::mutation_scope` |
|   0.8%     69.7 | `[unknown]` |
|   0.8%     68.9 | `<core_engine::store::KeyValueStore>::order_keys::<&str>` |
|   0.8%     68.5 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   0.8%     67.0 | `<core::str::lossy::Utf8Chunks as core::iter::traits::iterator::Iterator>::next` |
|   0.8%     66.6 | `core_engine::cmd::extract_key` |
|   0.8%     65.8 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   0.8%     65.4 | `core::ptr::drop_glue::<core::iter::adapters::zip::Zip<core::iter::adapters::zip::Zip<core::slice::iter::Iter<…` |
|   0.8%     65.4 | `libc.so.6:+175818` |
|   0.7%     63.9 | `core_engine::request::build_ref` |
|   0.7%     61.2 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|   0.7%     58.9 | `<str>::to_uppercase` |
|   0.7%     58.5 | `<alloc::vec::Vec<alloc::vec::Vec<u8>> as alloc::vec::spec_from_iter_nested::SpecFromIterNested<alloc::vec::Ve…` |
|   0.7%     58.1 | `<recached_server::persistence::AOF_DEFERRED::__KEY::{K#0}::{closure#0} as core::ops::function::FnOnce<(core::…` |
|   0.7%     58.1 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   0.7%     57.4 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   0.6%     55.4 | `core::ptr::drop_glue::<core_engine::store::SlotRef>` |
|   0.6%     54.3 | `<core_engine::request::Args>::general` |
|   0.6%     52.0 | `recached_server::propagation::is_write_command` |
|   0.6%     51.6 | `<core::iter::adapters::zip::Zip<core::iter::adapters::zip::Zip<core::slice::iter::Iter<&str>, smallvec::IntoI…` |
|   0.6%     49.7 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   0.5%     45.4 | `<alloc::string::String as core::clone::Clone>::clone` |
|   0.5%     45.0 | `tokio::runtime::task::waker::wake_by_val` |
|   0.5%     43.9 | `core_engine::store::entry_size` |
|   0.5%     43.9 | `core_engine::resp::format_i64` |
|   0.5%     43.1 | `tokio::runtime::task::waker::drop_waker` |
|   0.5%     39.3 | `libc.so.6:+139b40` |
|   0.4%     38.1 | `core_engine::cmd::extract_string` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%   8675.6 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  95.8%   8314.2 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  93.2%   8087.8 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  91.0%   7897.6 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  89.1%   7728.2 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  35.2%   3057.4 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  33.0%   2863.4 | `recached_server::clients::execute_and_record_ref` |
|  26.0%   2255.5 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  20.9%   1812.8 | `<core_engine::request::RequestDecoder>::decode_any` |
|  14.0%   1216.9 | `<core_engine::request::Args>::general` |
|  11.2%    968.2 | `<core_engine::cmd::Command>::from_value` |
|   8.0%    692.2 | `<core_engine::store::KeyValueStore>::execute_inner` |
|   7.9%    685.6 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   5.8%    502.4 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   5.2%    452.7 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   5.0%    436.2 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   4.3%    374.6 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   4.0%    348.0 | `<core_engine::cmd::Command as core::clone::Clone>::clone` |
|   4.0%    346.1 | `<core_engine::store::KeyValueStore>::entry_state` |
|   3.8%    325.7 | `<std::sys::pal::unix::time::Timespec>::now` |
|   3.7%    323.8 | `_rjem_malloc` |
|   3.7%    321.1 | `recached_server::clients::record_metrics` |
|   3.1%    270.6 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   2.8%    243.7 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   2.7%    232.5 | `core_engine::store::write_slot::<alloc::string::String>` |
|   2.5%    216.4 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   2.5%    216.0 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   2.4%    211.7 | `libc.so.6:+139b5b` |
|   2.3%    202.9 | `<tokio::runtime::time::Driver>::park_internal` |
|   2.2%    192.9 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   2.2%    191.7 | `core_engine::cmd::extract_key` |
|   2.2%    191.3 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   2.1%    182.9 | `<tokio::runtime::io::driver::Driver>::turn` |
|   1.9%    164.0 | `<core_engine::store::KeyValueStore>::order_keys::<&str>` |
|   1.9%    160.5 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   1.8%    157.8 | `<recached_server::persistence::DeferredAof>::pop` |
|   1.7%    151.3 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   1.7%    149.4 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   1.7%    143.6 | `<alloc::vec::Vec<alloc::vec::Vec<u8>> as core::clone::Clone>::clone` |
|   1.5%    134.0 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   1.5%    128.2 | `_rjem_sdallocx` |
|   1.4%    125.1 | `core_engine::cmd::extract_string` |
|   1.4%    124.0 | `libc.so.6:+1c7d91` |
|   1.4%    120.5 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   1.3%    115.1 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.3%    113.6 | `libc.so.6:+edc42` |
|   1.3%    113.6 | `std::sys::backtrace::__rust_begin_short_backtrace::<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::…` |
|   1.3%    113.6 | `libc.so.6:+17319c` |
|   1.3%    113.6 | `<std::sys::thread::unix::Thread>::new::thread_start` |
|   1.3%    113.6 | `<std::thread::lifecycle::spawn_unchecked<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::{closure#0}…` |
|   1.3%    110.1 | `<alloc::vec::Vec<alloc::vec::Vec<u8>> as alloc::vec::spec_from_iter_nested::SpecFromIterNested<alloc::vec::Ve…` |
|   1.2%    104.7 | `<alloc::string::String>::from_utf8_lossy` |
|   1.2%    102.4 | `[vdso]:+a8c` |
|   1.1%     97.8 | `core::ptr::drop_glue::<core::iter::adapters::zip::Zip<core::iter::adapters::zip::Zip<core::slice::iter::Iter<…` |
|   1.1%     96.6 | `core_engine::store::now_ms` |
|   1.0%     90.1 | `core_engine::resp::write_header` |
|   1.0%     88.2 | `core_engine::store::lock_stripe` |
|   1.0%     84.7 | `<alloc::string::String as core::clone::Clone>::clone` |
|   1.0%     83.9 | `core::ptr::drop_glue::<core_engine::store::SlotRef>` |
|   0.9%     78.5 | `recached_server::clients::command_name` |
