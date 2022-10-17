# Unreleased
* Update dependencies.

# v0.2.4 - 2021-11-30
* Fix adding files to the index of the index repository.

# v0.2.3 - 2021-10-05
* Fix deserialization of search queries with escaped characters.

# v0.2.2 - 2021-10-03
* Automatically reload TLS keys and certificates every 24 hours.

# v0.2.1 - 2021-09-12
* Add method and CLI command to completely delete a crate.

# v0.2.0 - 2021-09-02
* Add README.
* Implement `search` API endpoint.
* Add more logging to the server application and improve logging format.
* Create new registries with user provided URL in the CLI tool.

# v0.1.3 - 2021-08-30
* Use lower case `palletizer.toml` as configuration file.
* Fix format of dependencies in index entries.

# v0.1.2 - 2021-08-29
* Limit dependencies to registries allowed in the configuration file.

# v0.1.1 - 2021-08-28
* Fix index path for 1, 2 and 3 letter crates.
* Fix the name of the `version_req` field in index entries.

# v0.1.0 - 2021-05-17
* Support adding, yanking and unyanking crates.
* Serve the index and registry API on a webserver.
