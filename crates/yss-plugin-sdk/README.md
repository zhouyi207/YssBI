# Plugin Rust SDK

> Status: Current
> Scope: 双向 framed IPC、请求关联、队列预算与连接关闭
> Canonical owners: [Peer](src/lib.rs)
> Update when: 传输、预算、请求关联或连接生命周期改变时

Versioned bidirectional framed IPC for plugins and the host gateway. The SDK depends on the public
protocol package only, owns bounded queues/correlation, and accepts reduced grants during initialization.
It contains no Project, Graph, database, Julia or Bayes implementation.

The pending-request registry also owns connection closure: a closed registry cannot admit a new
request. Closing takes the registry under its lock and wakes existing callers after releasing it.
A request admitted before closure may already be dispatching; closure does not prove that its
remote operation was never executed. The process owner remains responsible for stopping transport.

An outbound call remains local until its frame enters the writer queue. Serialization failure,
an oversized frame, or a byte-budget or queue-capacity rejection retires only that call's
correlation; other admitted calls and later requests keep using the connection. Failed queue
admission releases its reserved byte charge. A disconnected writer instead reports
`plugin_process_exited`, closes the registry and wakes all admitted callers. Writer I/O failure
and a response that cannot be delivered also close the peer.

A request timeout closes this same registry and wakes all other admitted callers. The timed-out
caller receives `plugin_request_timeout`; other pending callers receive `plugin_process_exited`.
No expired-request cache is retained for a closed connection. Responses without a matching pending
request close the peer. The host process supervisor observes that closure and stops the transport.

Plugins reference this existing crate through an explicit Cargo path dependency.
The [native_form example](examples/native_form.rs) is a complete Rust process using initialization
budget grants, typed native command replies and cancellable tasks. Build it with
`cargo build --locked -p yss-plugin-sdk --example native_form`; the Runtime
[package example](../yss-plugin-runtime/examples/native_view_package.rs) creates its signed native
view package. Separate SDK publication is deferred until it has independent consumers.
