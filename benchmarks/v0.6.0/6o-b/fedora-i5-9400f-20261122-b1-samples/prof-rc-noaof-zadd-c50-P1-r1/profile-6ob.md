samples: 24721, user cycles/op 10649

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|  27.1%   2883.5 | `decode (all)` |
|  20.1%   2145.6 | `decode: general Value path (Args::general)` |
|  16.3%   1738.1 | `decode: from_value` |
|   0.0%      0.0 | `decode: build (owned MSET/DEL)` |
|   0.0%      0.0 | `to_command (borrowed -> owned)` |
|   0.0%      0.0 | `borrowed execute (try_execute_borrowed)` |
|  36.2%   3851.9 | `ordered write entry (execute_ordered_write)` |
|  34.5%   3673.5 | `store execute (all)` |
|   2.1%    218.8 | `store: Command clone` |
|   3.0%    314.5 | `store: key hash (sip)` |
|   3.1%    328.2 | `store: entry_state` |
|   1.2%    130.1 | `store: extract_key` |
|   2.1%    227.0 | `clock reads` |
|   2.9%    311.4 | `metrics record` |
|   0.2%     26.3 | `reply serialize (Value)` |
|   1.1%    122.3 | `Value drop` |
|   0.3%     29.7 | `Command drop` |
|   6.7%    717.2 | `allocator (all)` |
|   4.0%    428.2 | `select overflow arm (mpsc recv)` |
|   8.7%    930.9 | `BufWriter flush (all)` |
|   9.4%    999.8 | `AofGatedWriter (all)` |
|   1.5%    161.5 | `DeferredAof pop` |
|   2.4%    256.7 | `socket read (poll_read_priv)` |
|   3.5%    372.2 | `socket write (poll_write_priv)` |
|   9.4%   1000.2 | `split lock (ReadHalf/WriteHalf)` |
|   2.2%    234.3 | `tokio park/driver` |
|  96.7%  10294.7 | `tokio run_task (scheduler)` |
|   0.0%      0.0 | `task-local scope (with_deferred_aof)` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  34.3% |    3652 |
| connection (decode, reply, socket) | allocator |   6.2% |     659 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   3.2% |     343 |
| connection (decode, reply, socket) | tokio mpsc |   2.2% |     236 |
| connection (decode, reply, socket) | tokio wake/schedule |   1.0% |     108 |
| **connection (decode, reply, socket)** | **all** | ** 46.9%** | **   4997** |
| store execute | code |  24.7% |    2625 |
| store execute | libc (memcpy, syscall wrappers) |   5.7% |     604 |
| store execute | allocator |   0.9% |     100 |
| **store execute** | **all** | ** 31.3%** | **   3330** |
| tokio scheduler | tokio wake/schedule |   7.1% |     756 |
| tokio scheduler | code |   1.7% |     180 |
| tokio scheduler | tokio io/split |   1.6% |     172 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.2% |      16 |
| tokio scheduler | allocator |   0.0% |       1 |
| tokio scheduler | tokio mpsc |   0.0% |       0 |
| **tokio scheduler** | **all** | ** 10.6%** | **   1126** |
| AOF append/lead/gate | tokio io/split |   4.2% |     445 |
| AOF append/lead/gate | code |   3.1% |     327 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   1.9% |     203 |
| **AOF append/lead/gate** | **all** | **  9.2%** | **    975** |
| ordered write path (rest) | code |   1.7% |     178 |
| **ordered write path (rest)** | **all** | **  1.7%** | **    178** |
| tokio io driver | code |   0.4% |      38 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.0% |       4 |
| **tokio io driver** | **all** | **  0.4%** | **     43** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|   8.3%    884.4 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   6.8%    721.1 | `<core_engine::store::ZSetInner>::insert` |
|   4.7%    500.5 | `<core_engine::cmd::Command>::from_value` |
|   4.1%    441.5 | `_rjem_malloc` |
|   3.9%    420.4 | `<core_engine::request::RequestDecoder>::decode_any` |
|   3.3%    351.1 | `libc.so.6:+1c7919` |
|   3.2%    336.9 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   2.5%    261.0 | `_rjem_sdallocx` |
|   2.2%    239.1 | `core_engine::request::build_ref` |
|   2.0%    211.1 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   1.9%    207.6 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   1.7%    186.1 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   1.7%    182.6 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.6%    175.3 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   1.6%    172.7 | `<core::str::lossy::Utf8Chunks as core::iter::traits::iterator::Iterator>::next` |
|   1.6%    172.3 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   1.5%    162.8 | `recached_server::clients::record_metrics` |
|   1.5%    161.5 | `<recached_server::persistence::DeferredAof>::pop` |
|   1.4%    147.3 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   1.3%    134.0 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   1.2%    132.7 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   1.2%    131.8 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   1.2%    126.6 | `<core_engine::request::Args>::general` |
|   1.2%    126.2 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.2%    123.2 | `core::num::imp::dec2flt::parse::parse_number` |
|   1.1%    122.3 | `core::ptr::drop_glue::<core_engine::resp::Value>` |
|   1.1%    121.0 | `<core::iter::adapters::zip::Zip<core::iter::adapters::zip::Zip<core::slice::iter::Iter<&str>, smallvec::IntoI…` |
|   1.1%    115.4 | `[vdso]:+a8c` |
|   1.0%    109.0 | `libc.so.6:+1c7d91` |
|   1.0%    108.1 | `<core_engine::store::KeyValueStore>::execute_inner` |
|   1.0%    107.7 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   1.0%    101.7 | `core::str::converts::from_utf8` |
|   0.9%     99.9 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   0.9%     96.9 | `<str>::to_uppercase` |
|   0.9%     93.9 | `core::ptr::drop_glue::<alloc::vec::Vec<(f64, alloc::string::String)>>` |
|   0.8%     88.7 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   0.8%     85.3 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   0.8%     83.1 | `core_engine::store::zadd_exec` |
|   0.8%     81.4 | `<alloc::string::String>::from_utf8_lossy` |
|   0.8%     80.1 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   0.7%     77.5 | `tokio::runtime::task::waker::clone_waker` |
|   0.7%     74.1 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   0.7%     72.8 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   0.7%     71.5 | `core_engine::cmd::extract_string` |
|   0.6%     68.9 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   0.6%     65.9 | `core_engine::request::build` |
|   0.6%     65.9 | `<core_engine::store::KeyValueStore>::order_keys::<&str>` |
|   0.6%     65.5 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.6%     65.5 | `core_engine::store::lock_stripe` |
|   0.6%     60.7 | `<f64 as core::str::traits::FromStr>::from_str` |
|   0.6%     59.0 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   0.5%     56.9 | `[unknown]` |
|   0.5%     50.8 | `tokio::runtime::task::waker::wake_by_val` |
|   0.5%     50.8 | `<core_engine::cmd::Command as core::clone::Clone>::clone` |
|   0.5%     50.0 | `core_engine::store::write_slot::<alloc::string::String>` |
|   0.5%     49.5 | `libc.so.6:+175818` |
|   0.5%     48.2 | `core::ptr::drop_glue::<core::iter::adapters::zip::Zip<core::iter::adapters::zip::Zip<core::slice::iter::Iter<…` |
|   0.4%     47.4 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   0.4%     43.5 | `<hashbrown::map::HashMap<alloc::sync::Arc<str>, f64, std::hash::random::RandomState>>::insert` |
|   0.4%     38.3 | `<recached_server::persistence::AOF_DEFERRED::__KEY::{K#0}::{closure#0} as core::ops::function::FnOnce<(core::…` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%  10648.8 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  96.7%  10294.7 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  94.9%  10108.2 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  93.0%   9900.2 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  91.3%   9724.4 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  36.1%   3848.8 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  34.5%   3673.5 | `recached_server::clients::execute_and_record_ref` |
|  30.1%   3209.2 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  27.1%   2883.5 | `<core_engine::request::RequestDecoder>::decode_any` |
|  20.1%   2145.6 | `<core_engine::request::Args>::general` |
|  17.2%   1832.9 | `<core_engine::store::KeyValueStore>::execute_inner` |
|  16.3%   1738.1 | `<core_engine::cmd::Command>::from_value` |
|  12.7%   1350.0 | `core_engine::store::zadd_exec` |
|  11.8%   1255.7 | `<core_engine::store::ZSetInner>::insert` |
|   8.7%    930.9 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   6.4%    681.5 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   5.7%    608.7 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   4.1%    441.5 | `_rjem_malloc` |
|   4.0%    428.2 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   3.8%    402.8 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   3.5%    372.2 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   3.3%    351.1 | `libc.so.6:+1c7919` |
|   3.2%    343.3 | `core_engine::cmd::extract_string` |
|   3.1%    328.2 | `<core_engine::store::KeyValueStore>::entry_state` |
|   2.9%    311.4 | `recached_server::clients::record_metrics` |
|   2.5%    261.0 | `_rjem_sdallocx` |
|   2.4%    256.7 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   2.4%    255.9 | `core_engine::cmd::extract_float` |
|   2.4%    254.1 | `<alloc::string::String>::from_utf8_lossy` |
|   2.2%    239.1 | `core_engine::request::build_ref` |
|   2.2%    234.3 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   2.2%    229.2 | `core_engine::store::write_slot::<alloc::string::String>` |
|   2.1%    227.0 | `<std::sys::pal::unix::time::Timespec>::now` |
|   2.1%    227.0 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   2.1%    221.8 | `<tokio::runtime::time::Driver>::park_internal` |
|   2.1%    218.8 | `<core_engine::cmd::Command as core::clone::Clone>::clone` |
|   2.0%    212.8 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   1.9%    198.6 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   1.8%    196.9 | `<tokio::runtime::io::driver::Driver>::turn` |
|   1.8%    192.1 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   1.7%    183.9 | `<f64 as core::str::traits::FromStr>::from_str` |
|   1.7%    182.6 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.7%    180.9 | `libc.so.6:+139b5b` |
|   1.7%    179.2 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   1.6%    172.7 | `<core::str::lossy::Utf8Chunks as core::iter::traits::iterator::Iterator>::next` |
|   1.5%    161.5 | `<recached_server::persistence::DeferredAof>::pop` |
|   1.4%    154.2 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   1.4%    147.8 | `<core_engine::store::KeyValueStore>::order_keys::<&str>` |
|   1.4%    147.8 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   1.4%    147.3 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   1.3%    136.1 | `<str>::to_uppercase` |
|   1.2%    130.1 | `core_engine::cmd::extract_key` |
|   1.2%    127.5 | `<alloc::vec::Vec<(f64, alloc::string::String)> as core::clone::Clone>::clone` |
|   1.2%    125.4 | `core::ptr::drop_glue::<alloc::vec::Vec<(f64, alloc::string::String)>>` |
|   1.2%    123.2 | `core::num::imp::dec2flt::parse::parse_number` |
|   1.1%    122.3 | `core::ptr::drop_glue::<core_engine::resp::Value>` |
|   1.1%    121.0 | `<core::iter::adapters::zip::Zip<core::iter::adapters::zip::Zip<core::slice::iter::Iter<&str>, smallvec::IntoI…` |
|   1.1%    115.4 | `[vdso]:+a8c` |
|   1.0%    109.0 | `libc.so.6:+1c7d91` |
|   1.0%    101.7 | `core::str::converts::from_utf8` |
