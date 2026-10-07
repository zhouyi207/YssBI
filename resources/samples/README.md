# Bundled sample assets

> Status: Current
> Scope: Bundled dataset provenance and offline resource preparation
> Canonical owners: `sources.json` pins CSV inputs; `catalog.json` admits versioned Parquet payloads
> Update when: Sources, sample versions, preparation or distribution change

The native application needs only `catalog.json` and the versioned `data.parquet`
files. They are checked in so development startup and sample import work offline.
CSV inputs are an ignored preparation cache. They are downloaded explicitly from
the pinned commits and verified before replacement; startup never downloads data.

Run these commands from the repository root:

```sh
# Regenerate only when deliberately preparing new sample resources.
python3 scripts/samples/fetch_sources.py
cargo run -p yss-application --example build_samples

# Verify the shipped catalog and every payload; CSV inputs are optional.
cargo run -p yss-application --example build_samples -- --check

# Place runtime assets beside a debug native executable.
cargo run -p yss-application --example build_samples -- --check --stage target/debug/resources/samples
```

The preparation manifest lives at `scripts/samples/sources.json` in the repository;
staging includes a copy as `sources.json`. It owns IDs, display names, versions,
source URLs and SHA-256 values. Change an existing sample's version when changing
its source. The generator uses the existing Rust CSV and Parquet readers. Removing
an input timezone retains the input calendar and wall-clock fields, without
converting them to another timezone. All upstream columns are preserved, including
the Rdatasets `rownames` column.

## Sources and attribution

| Sample | Upstream source | Attribution and upstream notices |
| --- | --- | --- |
| Iris | [seaborn-data](https://github.com/mwaskom/seaborn-data), originally [UCI Iris](https://archive.ics.uci.edu/dataset/53/iris) | R. A. Fisher's Iris dataset; UCI source provides its dataset citation and license. |
| Diamonds | [seaborn-data](https://github.com/mwaskom/seaborn-data), from [ggplot2 diamonds](https://ggplot2.tidyverse.org/reference/diamonds.html) | Prices and measurements of 53,940 diamonds; ggplot2 provides the dataset reference. |
| Flights | [Rdatasets / nycflights13](https://vincentarelbundock.github.io/Rdatasets/doc/nycflights13/flights.html) | NYC departures in 2013, Bureau of Transportation Statistics; the [nycflights13 package](https://github.com/tidyverse/nycflights13/blob/main/DESCRIPTION) declares CC0. |
| dataCar | [Rdatasets / insuranceData](https://vincentarelbundock.github.io/Rdatasets/doc/insuranceData/dataCar.html) | De Jong and Heller's car insurance data, prepared by Alicja Wolny-Dominiak and Michal Trzesiok; the [insuranceData package](https://github.com/cran/insuranceData/blob/master/DESCRIPTION) declares GPL-2. |
| vic_elec | [Rdatasets / tsibbledata](https://vincentarelbundock.github.io/Rdatasets/doc/tsibbledata/vic_elec.html) | Victorian half-hour electricity demand, temperature and holidays, sourced from AEMO; the [tsibbledata package](https://github.com/tidyverts/tsibbledata/blob/master/DESCRIPTION) declares GPL-3. |

Pinned download locations are recorded in the preparation manifest. Upstream
package notices and dataset references remain with their respective owners;
preparation does not replace them with an application-wide data license.
