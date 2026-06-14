# CONTRIBUTING.md

# Contributing to AURELIUM

Thank you for your interest in contributing.

## Development Principles

* Simplicity over complexity
* Reliability over novelty
* Observability by default
* Security first
* Evolution without regressions

## Workflow

1. Fork the repository
2. Create a feature branch

```bash
git checkout -b feature/my-feature
```

3. Commit changes

```bash
git commit -m "feat: add capability"
```

4. Push branch

```bash
git push origin feature/my-feature
```

5. Open a Pull Request

## Pull Request Requirements

* Tests must pass
* Documentation must be updated
* Security implications documented
* Architecture impact described

## Commit Convention

```text
feat:
fix:
docs:
test:
refactor:
perf:
security:
infra:
```

## Architecture Decision Records

Major architectural changes must include an ADR under:

```text
docs/adr/
```

## Code Review

Every contribution requires at least one maintainer approval.

Thank you for helping build the Living Software Fabric.
