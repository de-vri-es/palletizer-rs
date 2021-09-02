# Palletizer

Palletizer is a fully open-source (BSD-2-clause) self-hosted private Cargo registry.
The main target audience is small-scale organizations.

It favors simplicity over scalability: you can host your entire registry by running a single server application.
That said, it should run pretty smooth with a low memory footprint for most organizations,
and performance problems will be taken seriously.

Current features:
* Host your crate data and index repository from the same server.
* Publish and yank crates using the Cargo web API.
* Search for crates using `cargo search --registry ...`.
* Multiple listening sockets for the web server, each with independent (optional) TLS configuration.

# Authentication

At the moment, Palletizer does not implement authentication.
The API server does not check for login tokens, even though Cargo still wants to you specify one.
You can use any random gibberish as token, the server doesn't even look at it.

The intention is certainly to implement secure authentication so that you can expose your registry to the Internet safely,
but the main obstacle at the moment is Cargo.
At the time of writing, it can not perform authenticated index or crate downloads.
The login token is only used to access the web API,
but that is not good enough for a private registry that hosts potentially sensitive code.

Until authentication is available for crate downloads, Palletizer will not support authentication.
This should avoid giving the false impression that your code is safe if you simply configure authentication.
See [RFC #2719] for more information.

[RFC #2719]: https://github.com/rust-lang/rfcs/pull/2719

# Ok, but I really do want authentication

Fair enough, that makes sense for your private registry full of private code.
For now, you can put a proxy (for example: [nginx]) in front of the web server to perform authentication.
However, you need to carefully choose your authentication mechanism if you want it to work with vanilla Cargo.
One thing that could work is to use IP based authentication to allow access from certain networks.

You can also bind the Palletizer server to a private VPN address directly and avoid the need for a separate proxy server.
If you're looking for a good and simple VPN, you could take a look at [WireGuard].

[nginx]: https://nginx.org/
[WireGuard]: https://wireguard.com/

# Project structure

The project consists of a library, a command line tool and a server application.
The library could be used to implement different front-ends for Palletizer registries.
It is also used by the command line tool and the server application to implement the actual registry management.

The command line tool does not communicate with a running server.
It can be used to add, remove, yank and unyank crates from a registry manually.
You do need direct access to the registry in order to use it.

The server application hosts the index repository, the crate data and the Cargo web API.
You could also decide to use a dedicated server for the crate data and the index repository,
and only expose the API of the Palletizer server to the Internet (or your VPN).

# Contributing

Contributions are always welcome.
Feel free to open an issue or pull request on GitHub.
