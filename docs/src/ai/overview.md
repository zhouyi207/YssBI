# Assistant

The Assistant works with the current project through YssBI's typed tools. Use Ask to inspect and discuss existing information, or Write when you want it to change resources or run an analysis.

## Configure a model

1. Open Tools → Settings, or press `Ctrl+,` (`Cmd+,` on macOS).
2. In AI, open the model-provider configuration and add a provider. Select the appropriate preset or custom protocol, then check its endpoint and authentication settings.
3. Supply an API key when required. An OpenAI-compatible local service can instead use `none` authentication if the service accepts unauthenticated requests.
4. Add a model by its service model ID, or use model discovery if supported by that connection. Discovery merges models into the draft; it does not save the connection or credentials.
5. Save the provider and choose a default model. You can also select a configured model for an individual conversation.

API keys are stored in the system credential store. The application configuration contains credential references, not the secret itself; keys are not written to the project, conversation events, or logs. An empty key input preserves an existing key. A replacement key replaces it when the configuration is committed.

Model options include optional sampling parameters and protocol-native JSON parameters. Leave sampling fields empty to use the provider's defaults. Protocol and model support vary; configuring a field does not establish that a provider accepts it.

> **Note:** Using a remote model sends conversation and tool context to the configured service. Ask is read-only with respect to project tools, not an offline or no-data-disclosure mode. Use data and services appropriate for your privacy requirements.

## Start a conversation

Open View → Assistant, then create a conversation or open one from the conversation directory. Conversations belong to a project; switching projects does not grant an old conversation permission to operate on the new one.

Select a model, choose Ask or Write, and add project resource references to make your target explicit. Enter sends a message; Shift+Enter inserts a newline. Escape requests a stop. Closing a conversation panel alone does not cancel its active work.

A useful first request identifies the resource and the question, for example: “Inspect this graph's inputs and explain what prevents it from running.” Start in Ask mode to review the project before allowing changes. Use a copied project for Write-mode experiments.

## Ask and Write

| Mode  | Allowed behavior                                                                                                                                                                                                                       |
| ----- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Ask   | Inspection-only capabilities. It can read permitted project facts and existing results, but cannot run graph computation, write resources, export data, or issue UI-opening intents. Delegated tasks cannot bypass these restrictions. |
| Write | May use mutation, execution, export, and UI capabilities when its role, task scope, and project authorization allow them. It is not unrestricted filesystem or application access.                                                     |

Mode and reasoning effort are captured when a message is submitted. Changing the current controls does not rewrite a running turn, queued message, or historical turn. Changing a conversation's model affects later turns, not an already-running client.

Reasoning effort offers the model's configured choices, or Low, Medium, and High when no explicit restriction is configured. The provider still determines whether a requested level is supported; the control does not guarantee a particular model's behavior or expose reasoning that the provider does not return.

## Understand tool activity

The Assistant can discover available nodes, inspect graphs and resources, read existing result pages, and use registered editing and execution tools within its permissions. It coordinates a Manager and scoped Workers; task and tool cards show their recorded activity, failures, and outputs.

Expand tool details to inspect the target, safe argument summary, result references, or failure code. Distinguish a tool call's status from a graph run's status. Missing timing or token reports mean unknown or incomplete information, not zero work. An outcome marked unknown is not proof that nothing changed: inspect the affected resource before requesting the same change again.

Write-mode graph-editing tools can save their successful changes as part of the transaction. By contrast, graph validation and execution do not implicitly save; graph undo/redo also does not automatically save. The Assistant shares the same project graph and history as the desktop, not an isolated draft. Ask it to explain the intended change before switching to Write if you need to review the plan first.

Resource and result cards open the referenced item when it is still available. A deleted resource or released result can remain visible in history without being usable. Conversation replay restores recorded activity; it is not a request to reexecute the historical tools.

## Limits

The Assistant is not a general shell, an external MCP server, or a guaranteed unattended agent. A registered node or tool does not imply that every model will discover or use it correctly. Review numerical results, study design, and conclusions yourself; fluent prose is not evidence that an analysis succeeded.

Native real-model acceptance remains open for tool and task presentation, cancellation, replay, stale evidence, and Manager–Worker delivery. Draft persistence across restarts and other conversation lifecycle interactions also remain incomplete. Do not treat visible history as a durable backup of unsent input or as authorization for future writes.

See [Graphs and Results](../graphs-and-results.md) for execution and snapshots, the [Harness boundary](../development/architecture.md#harness-boundary) for ownership, and the [Harness contract](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-harness-core/README.md) for the current capability and provider definitions. Remaining work is tracked under [Assistant and Harness](https://github.com/zhouyi207/YssBI/blob/main/TODO.md#assistant-and-harness).
