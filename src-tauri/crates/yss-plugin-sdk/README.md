# Plugin Rust SDK

Versioned bidirectional framed IPC for plugins and the host gateway. The SDK depends on the public
protocol package only, owns bounded queues/correlation, and accepts reduced grants during initialization.
It contains no Project, Graph, database, Julia or Bayes implementation.

Plugins reference this existing crate through an explicit Cargo path dependency. The Web SDK remains
inside the Julia plugin; separate SDK publication is deferred until it has independent consumers.
