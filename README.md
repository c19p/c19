# The C19 Protocol
The C19 protocol is a variant of the [Gossip protocol](https://en.wikipedia.org/wiki/Gossip_protocol). It allows a group of services to agree on a service-wide state.

The state is shared across a distributed set of services which in effect means that each service has the data available locally.

![Sharing state use case](resources/sharing-state.png)

C19 decouples the process of fetching the data from using it. Consider a case of two or more microservices with different dependencies. 
A dependant service would have to handle fetching the data from other services. It has to consider cases of latency and unavailability. 
But fetching the data is not its main focus and should not be it's main concern. By decoupling fetching the data from using it, C19 makes sure 
the data is available locally to the service. 

If you are running a microservices architecture you will soon, if not already, find yourself having to deal with dependencies across services, handling unavailability, scale, 
redundancy, etc. Microservices architecture brings with it a lot of complexities that are not at the core of your service. 

C19 is a simple, powerful and extensible system and can reduce the complexities by taking care of fetching the data and making it available locally to your services.

## The Books
The best and most extensive source of information is the [User Guide]. Please read it!
It has anything from a step by step guide for running the C19 protocol to a drill down on architecture.

[The User Guide]

And we have a second book ready if you wish to contribute to the C19 project.

[The Developer Guide]

## Kubernetes
While the C19 protocol can be run anywhere, we target most of our use cases to Kubernetes since it is the most common way of running microservices architecture and simplifies 
the way discovery works.

The C19 protocol has different deployment strategies. Please refer to the user guide [Deployment Strategies] section for more information.

## Getting Started
Please refer to the [Getting Started] section on the user guide for an extensive step-by-step guide on how to deploy and run the protocol with or without a Kubernetes cluster.

## Motivation and Use Cases
The [Motivation] and [Use Cases] section on the user guide will guide you through our reasoning and the different use cases we believe can be solved by the C19 protocol.

## Contributing
Every contribution matters! And we greatly appreciate any help in improving and extending the C19 protocol. Please refer to the [Contributing](CONTRIBUTING.md) file in this repository or the 
[Contributing] section on the [Developer Guide].

## License
BSD-3-Clause.

[The User Guide]: https://c19p.github.io/user-guide/title-page.html
[User Guide]: https://c19p.github.io/user-guide/title-page.html
[The Developer Guide]: https://c19p.github.io/developer-guide/
[Developer Guide]: https://c19p.github.io/developer-guide/
[Deployment Strategies]: https://c19p.github.io/user-guide/deployment-strategies.html
[Getting Started]: https://c19p.github.io/user-guide/ch01-00-getting-started.html
[Contributing]: https://c19p.github.io/developer-guide/contributing.html
[Motivation]: https://c19p.github.io/user-guide/motivation.html
[Use Cases]: https://c19p.github.io/user-guide/use-cases.html
