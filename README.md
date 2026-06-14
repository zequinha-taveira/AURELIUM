# AURELIUM – Living Software Fabric

AURELIUM is a next-generation decentralized agentic software platform. It functions as a living software fabric where autonomous agents, reactive runtimes, and world simulators collaborate to create self-evolving, reliable systems.

---

## 🏛️ Project Architecture

The repository is organized as a monorepo managed by [Turborepo](https://turbo.build/) to optimize build caching and project orchestration.

```text
AURELIUM/
├── .github/              # GitHub Actions & CI/CD configuration workflows
├── apps/                 # Client and server-side end-user applications
├── core/                 # Core logic, fabric specifications, and base protocols
├── agents/               # Intelligent, autonomous, and self-improving agents
├── runtime/              # Execution environments and VM containerization layers
├── simulation/           # Reality simulation, future-world modeling, and scenario testing
├── infrastructure/       # IaC, Kubernetes manifests, and orchestration configs
├── packages/             # Shared libraries, utilities, and internal packages
├── docs/                 # Architectural documentation and Architecture Decision Records (ADRs)
└── tests/                # System-wide integration and end-to-end verification suites
```

---

## ⚡ Quick Start

### Prerequisites

* [Node.js](https://nodejs.org/) v18+ & [pnpm](https://pnpm.io/)
* [Docker](https://www.docker.com/) & Docker Compose
* [Rust](https://www.rust-lang.org/) (for high-performance runtime components)
* [Python](https://www.python.org/) 3.10+ (for agent model training and simulation tools)

### 1. Boot Infrastructure Dependencies

Start postgres, neo4j, nats, qdrant, and minio containers:

```bash
docker compose -f docker-compose.dev.yml up -d
```

### 2. Install Dependencies

Install Node.js dependencies across all monorepo packages:

```bash
pnpm install
```

### 3. Run Development Server

Run all applications and packages in development mode:

```bash
pnpm dev
```

---

## 🎯 Development Principles

All contributions to AURELIUM should align with our core principles:

1. **Simplicity over Complexity**: Prioritize clear, simple designs. If a component is hard to explain, it needs refactoring.
2. **Reliability over Novelty**: Favor mature, well-tested technologies and solid APIs.
3. **Observability by Default**: All systems must emit rich telemetry (metrics, logs, traces) for debugging.
4. **Security First**: Zero trust architecture, least privilege access control, and strict input sanitization.
5. **Evolution without Regressions**: Code updates must not degrade the performance or stability of the living fabric.

---

## 📖 Documentation & ADRs

We document significant architectural decisions via Architecture Decision Records (ADRs). When proposing a change that changes the behavior or design of a system component:
1. Create a new markdown file under `docs/adr/XXXX-title.md`.
2. Follow the standard ADR format (Context, Decision, Consequences).
3. Refer to [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

---

## ⚖️ License

AURELIUM is licensed under the **Business Source License 1.1** (BSL 1.1).
* **Licensor**: Aurelium Labs
* **Licensed Work**: AURELIUM – Living Software Fabric
* **Change Date**: January 1, 2030
* **Change License**: Apache License 2.0

Additional Use Grant: The Licensed Work may be used, copied, modified, and distributed for personal, educational, research, and non-commercial experimentation use without restriction. Commercial use requires a separate agreement.

For full terms, see [BSL.md](BSL.md) and [LICENSE](LICENSE).
