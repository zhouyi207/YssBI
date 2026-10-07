# Plugin Rust SDK

Versioned bidirectional framed IPC for plugins and the host gateway. The SDK depends on the public
protocol package only, owns bounded queues/correlation, and accepts reduced grants during initialization.
It contains no Project, Graph, database, Julia or Bayes implementation.

The pending-request registry also owns connection closure: a closed registry cannot admit a new
request. Closing takes the registry under its lock and wakes existing callers after releasing it.
A request admitted before closure may already be dispatching; closure does not prove that its
remote operation was never executed. The process owner remains responsible for stopping transport.

Plugins reference this existing crate through an explicit Cargo path dependency. The Web SDK remains
inside the Julia plugin; separate SDK publication is deferred until it has independent consumers.
