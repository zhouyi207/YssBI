# Panel DID (TWFE)

Fit the response against optional predictors and a precomputed binary treatment interaction, Treat × Post, absorbing entity and time effects. Connect aligned finite numeric response, predictors, entity, time and treatment columns. Entity-time pairs must be unique.

The treatment input is the interaction, not the group indicator alone. Inference clusters by entity. The single result output contains model and summary, retaining the fitted model and its structured summary.

For fake-group randomization, use DID fake-group randomization with separate treat/post inputs. Event-study estimation is not exposed by this node.
