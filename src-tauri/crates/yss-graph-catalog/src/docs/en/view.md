# View

Open a data inspector sub-window for the connected value. Supports **DataFrame**, **DataSeries**, and scalar types. Read-only: viewing does not mutate project data.

## Usage

Wire **Data** from a pipeline output you want to inspect. View is an explicit graph result, so running the graph evaluates its upstream dependency closure. Large tables and series load via paginated typed APIs.
