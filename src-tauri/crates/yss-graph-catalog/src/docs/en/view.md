# View

Open a data inspector sub-window for the connected value. Supports **DataFrame**, **DataSeries**, and scalar types. Read-only: viewing does not mutate project data.

## Usage

Wire **Data** from a pipeline output you want to inspect, and place **View** on an exec path so it runs at the right time. Large tables and series load via paginated typed APIs. Chain **Out** to continue execution after inspection.
