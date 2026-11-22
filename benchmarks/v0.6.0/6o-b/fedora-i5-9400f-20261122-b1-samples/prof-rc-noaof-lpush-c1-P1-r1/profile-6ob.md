samples: 23949, user cycles/op 17902

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|  19.9%   3556.5 | `decode (all)` |
|  14.5%   2588.6 | `decode: general Value path (Args::general)` |
|  11.9%   2128.1 | `decode: from_value` |
|   0.0%      0.0 | `decode: build (owned MSET/DEL)` |
|   0.0%      0.0 | `to_command (borrowed -> owned)` |
|   0.0%      0.0 | `borrowed execute (try_execute_borrowed)` |
|  28.8%   5150.2 | `ordered write entry (execute_ordered_write)` |
|  26.6%   4762.2 | `store execute (all)` |
|   2.9%    515.0 | `store: Command clone` |
|   2.4%    432.0 | `store: key hash (sip)` |
|   4.3%    772.9 | `store: entry_state` |
|   1.9%    344.6 | `store: extract_key` |
|   5.3%    950.1 | `clock reads` |
|   3.0%    528.5 | `metrics record` |
|   0.3%     47.8 | `reply serialize (Value)` |
|   1.5%    276.6 | `Value drop` |
|   0.7%    118.9 | `Command drop` |
|   4.8%    853.6 | `allocator (all)` |
|   2.7%    480.6 | `select overflow arm (mpsc recv)` |
|   6.3%   1125.0 | `BufWriter flush (all)` |
|   7.4%   1333.5 | `AofGatedWriter (all)` |
|   1.0%    177.2 | `DeferredAof pop` |
|   1.9%    345.3 | `socket read (poll_read_priv)` |
|   1.8%    324.4 | `socket write (poll_write_priv)` |
|   7.4%   1333.5 | `split lock (ReadHalf/WriteHalf)` |
|  12.0%   2156.5 | `tokio park/driver` |
|  79.8%  14287.5 | `tokio run_task (scheduler)` |
|   0.0%      0.0 | `task-local scope (with_deferred_aof)` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  31.0% |    5547 |
| connection (decode, reply, socket) | allocator |   3.9% |     691 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   3.0% |     538 |
| connection (decode, reply, socket) | tokio mpsc |   1.3% |     228 |
| connection (decode, reply, socket) | tokio wake/schedule |   0.8% |     136 |
| **connection (decode, reply, socket)** | **all** | ** 39.9%** | **   7141** |
| tokio scheduler | tokio wake/schedule |  10.8% |    1925 |
| tokio scheduler | code |   8.8% |    1578 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   3.4% |     609 |
| tokio scheduler | tokio io/split |   0.8% |     137 |
| tokio scheduler | allocator |   0.0% |       1 |
| **tokio scheduler** | **all** | ** 23.7%** | **   4249** |
| store execute | code |  20.1% |    3589 |
| store execute | libc (memcpy, syscall wrappers) |   2.2% |     400 |
| store execute | allocator |   0.9% |     161 |
| **store execute** | **all** | ** 23.2%** | **   4151** |
| AOF append/lead/gate | tokio io/split |   4.6% |     815 |
| AOF append/lead/gate | code |   2.3% |     407 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   1.1% |     203 |
| **AOF append/lead/gate** | **all** | **  8.0%** | **   1425** |
| tokio io driver | code |   1.7% |     303 |
| tokio io driver | libc (memcpy, syscall wrappers) |   1.4% |     244 |
| **tokio io driver** | **all** | **  3.1%** | **    548** |
| ordered write path (rest) | code |   2.2% |     388 |
| **ordered write path (rest)** | **all** | **  2.2%** | **    388** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|   7.2%   1293.9 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   4.6%    820.0 | `<core_engine::cmd::Command>::from_value` |
|   3.7%    669.0 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   3.2%    569.6 | `<core_engine::request::RequestDecoder>::decode_any` |
|   3.0%    540.4 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   2.5%    447.0 | `_rjem_malloc` |
|   2.2%    402.1 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   2.1%    370.8 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   2.0%    352.8 | `<core_engine::store::KeyValueStore>::execute_inner` |
|   1.9%    337.1 | `_rjem_sdallocx` |
|   1.8%    317.7 | `recached_server::clients::record_metrics` |
|   1.7%    308.7 | `[vdso]:+a8c` |
|   1.7%    300.5 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   1.6%    281.8 | `<tokio::runtime::io::driver::Driver>::turn` |
|   1.5%    276.6 | `core::ptr::drop_glue::<core_engine::resp::Value>` |
|   1.3%    237.7 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.3%    229.5 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.3%    228.7 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   1.2%    219.0 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   1.1%    204.8 | `libc.so.6:+1c7d91` |
|   1.1%    198.1 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   1.1%    195.8 | `<core::str::lossy::Utf8Chunks as core::iter::traits::iterator::Iterator>::next` |
|   1.1%    194.3 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   1.1%    191.4 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   1.1%    188.4 | `<core_engine::store::KeyValueStore>::order_keys::<&str>` |
|   1.0%    178.6 | `<str>::to_uppercase` |
|   1.0%    177.9 | `<core_engine::cmd::Command as core::clone::Clone>::clone` |
|   1.0%    177.9 | `core_engine::request::build_ref` |
|   1.0%    177.2 | `<recached_server::persistence::DeferredAof>::pop` |
|   1.0%    174.9 | `<tokio::runtime::time::Driver>::park_internal` |
|   0.9%    167.4 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   0.9%    166.7 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|   0.9%    163.0 | `<tokio::process::imp::GlobalOrphanQueue>::reap_orphans` |
|   0.9%    153.2 | `tokio::runtime::task::waker::clone_waker` |
|   0.9%    153.2 | `<core_engine::request::Args>::general` |
|   0.8%    151.7 | `recached_server::propagation::is_write_command` |
|   0.8%    150.2 | `core_engine::cmd::extract_key` |
|   0.8%    144.3 | `core_engine::store::lock_stripe` |
|   0.8%    141.3 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   0.8%    136.8 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   0.8%    136.0 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   0.7%    133.8 | `<core_engine::resp::Value>::parse_inner` |
|   0.7%    130.8 | `core_engine::store::mutation_scope` |
|   0.7%    129.3 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   0.7%    124.1 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   0.7%    122.6 | `core::str::converts::from_utf8` |
|   0.7%    118.9 | `core_engine::store::entry_size` |
|   0.6%    115.1 | `tokio::runtime::task::raw::schedule::<alloc::sync::Arc<tokio::runtime::scheduler::multi_thread::handle::Handl…` |
|   0.6%    111.4 | `<recached_server::persistence::AOF_DEFERRED::__KEY::{K#0}::{closure#0} as core::ops::function::FnOnce<(core::…` |
|   0.6%    108.4 | `<tokio::runtime::time::wheel::Wheel>::next_expiration` |
|   0.6%    108.4 | `core::ptr::drop_glue::<core::iter::adapters::zip::Zip<core::iter::adapters::zip::Zip<core::slice::iter::Iter<…` |
|   0.6%    107.6 | `<std::sys::pal::unix::time::Timespec>::sub_timespec` |
|   0.6%    101.7 | `tokio::runtime::task::waker::wake_by_val` |
|   0.6%    100.2 | `core_engine::resp::format_i64` |
|   0.6%    100.2 | `<alloc::string::String>::from_utf8_lossy` |
|   0.6%     98.7 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   0.5%     96.4 | `core_engine::cmd::extract_string` |
|   0.5%     94.9 | `[unknown]` |
|   0.5%     94.2 | `<tokio::runtime::scheduler::multi_thread::stats::Stats>::end_processing_scheduled_tasks` |
|   0.5%     94.2 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%  17901.6 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  79.8%  14287.5 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  78.5%  14058.0 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  77.3%  13839.0 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  74.4%  13323.2 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  28.7%   5133.7 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  26.6%   4762.2 | `recached_server::clients::execute_and_record_ref` |
|  22.1%   3958.7 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  19.9%   3556.5 | `<core_engine::request::RequestDecoder>::decode_any` |
|  14.5%   2588.6 | `<core_engine::request::Args>::general` |
|  12.0%   2156.5 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|  11.9%   2128.1 | `<core_engine::cmd::Command>::from_value` |
|  10.2%   1826.9 | `<tokio::runtime::time::Driver>::park_internal` |
|   6.3%   1125.0 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   5.7%   1028.5 | `<tokio::runtime::io::driver::Driver>::turn` |
|   5.4%    971.7 | `<core_engine::store::KeyValueStore>::execute_inner` |
|   5.3%    950.1 | `<std::sys::pal::unix::time::Timespec>::now` |
|   4.6%    830.5 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   4.4%    789.3 | `<tokio::runtime::scheduler::multi_thread::stats::Stats>::end_processing_scheduled_tasks` |
|   4.3%    772.9 | `<core_engine::store::KeyValueStore>::entry_state` |
|   4.0%    714.6 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   4.0%    713.9 | `libc.so.6:+139b5b` |
|   3.8%    689.2 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   3.0%    540.4 | `std::sys::backtrace::__rust_begin_short_backtrace::<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::…` |
|   3.0%    540.4 | `<std::sys::thread::unix::Thread>::new::thread_start` |
|   3.0%    540.4 | `libc.so.6:+edc42` |
|   3.0%    540.4 | `libc.so.6:+17319c` |
|   3.0%    540.4 | `<std::thread::lifecycle::spawn_unchecked<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::{closure#0}…` |
|   3.0%    528.5 | `recached_server::clients::record_metrics` |
|   2.9%    515.0 | `<core_engine::cmd::Command as core::clone::Clone>::clone` |
|   2.7%    480.6 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   2.7%    480.6 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   2.5%    447.0 | `_rjem_malloc` |
|   2.1%    375.2 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   2.0%    357.3 | `libm.so.6:+c8227` |
|   2.0%    356.6 | `tokio::runtime::task::waker::wake_by_val` |
|   2.0%    352.1 | `<core_engine::store::KeyValueStore>::order_keys::<&str>` |
|   1.9%    345.3 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   1.9%    344.6 | `core_engine::cmd::extract_key` |
|   1.9%    337.1 | `_rjem_sdallocx` |
|   1.8%    324.4 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   1.7%    308.7 | `[vdso]:+a8c` |
|   1.7%    296.0 | `<alloc::string::String>::from_utf8_lossy` |
|   1.5%    276.6 | `core::ptr::drop_glue::<core_engine::resp::Value>` |
|   1.4%    252.7 | `core_engine::store::write_slot::<alloc::string::String>` |
|   1.4%    252.7 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   1.3%    237.7 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.3%    231.7 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.2%    223.5 | `core_engine::resp::write_header` |
|   1.2%    216.8 | `<str>::to_uppercase` |
|   1.2%    214.5 | `<alloc::vec::Vec<alloc::vec::Vec<u8>> as core::clone::Clone>::clone` |
|   1.2%    214.5 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   1.2%    208.5 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   1.1%    204.8 | `libc.so.6:+1c7d91` |
|   1.1%    195.8 | `<core::str::lossy::Utf8Chunks as core::iter::traits::iterator::Iterator>::next` |
|   1.1%    194.3 | `core_engine::cmd::extract_string` |
|   1.1%    189.9 | `core_engine::store::now_ms` |
|   1.0%    177.9 | `core_engine::request::build_ref` |
|   1.0%    177.2 | `<recached_server::persistence::DeferredAof>::pop` |
|   0.9%    166.7 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
