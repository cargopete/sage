# Sage Vision & Future Direction

This document captures the vision for Sage and where the language might evolve.

## The Bet

Sage is a bet that AI-native applications will increasingly be composed of **agents** - autonomous reasoning entities that coordinate to solve problems. Today, most production AI systems are single-agent or simple pipelines. Tomorrow, that might change.

## Where Multi-Agent Makes Sense

| Use Case | Why Multi-Agent? |
|----------|------------------|
| **Research with verification** | One agent researches, another fact-checks. Reduces hallucination through adversarial validation. |
| **Red team / blue team** | Adversarial testing of outputs. One agent generates, another attacks. |
| **Complex analysis** | Different "perspectives" on the same data. Financial analyst vs risk assessor vs compliance checker. |
| **Simulation** | Economic actors, game theory, social dynamics. Agents with beliefs making decisions. |
| **Content pipelines** | Writer → Editor → Fact-checker. Each stage specialised, maintaining quality. |

## Where Sage Fits Today

The honest reality: **single-agent + tools** is the dominant production pattern right now.

```
User → Agent → Tools → Response
```

This is what Claude Code does. This is what's shipping and working.

Sage currently emphasises multi-agent coordination (spawn, await, send). But the language could evolve to also excel at the single-agent + tools pattern that dominates today.

## Future Goals

### 1. First-Class Tool Support ✓

**Implemented in v0.5.0 (RFC-0011)**

Agents can now use built-in tools with the `use` declaration:

```sage
agent Fetcher {
    use Http

    on start {
        let response = try Http.get("https://api.example.com/data");
        print(response.body);
        emit(response.status);
    }

    on error(e) {
        emit(-1);
    }
}

run Fetcher;
```

The `Http` tool provides `get` and `post` methods. Future tools (Fs, Kv, Database, Browser) will follow the same pattern. MCP integration is a potential future direction.

### 2. Tool Discovery & Composition

Agents should be able to discover available tools and compose them dynamically:

```sage
// Future syntax
agent Orchestrator {
    on start {
        let tools = discover_tools()
        let plan = infer("Given these tools: {tools}, how would you solve: {self.task}")
        // ...
    }
}
```

### 3. Improved Concurrency Primitives

Beyond spawn/await, consider:
- Agent supervision trees (Erlang-style)
- Broadcast messaging
- Agent pools for load distribution
- Graceful shutdown and restart

### 4. Persistence & Checkpointing

Long-running agents need state persistence:

```sage
// Future syntax
agent LongRunningResearcher {
    @persistent
    belief findings: List<String>

    // State survives restarts
}
```

### 5. Observability

Production systems need visibility:
- Agent lifecycle events
- Message traces
- Belief state inspection
- Performance metrics

## Target Users

### Today
- AI researchers exploring multi-agent architectures
- Educators teaching agent-based concepts
- Language design enthusiasts

### Tomorrow (if the bet pays off)
- AI-native startups building agent-based products
- Teams building complex AI orchestration systems
- Developers who outgrow Python agent frameworks

## Non-Goals

Sage is **not** trying to be:
- A general-purpose backend language (use Rust/Go/Python)
- A web framework
- A replacement for traditional programming

It's a domain-specific language for AI agent systems. That's a narrow but potentially deep niche.

## Open Questions

1. **Should Sage compile to something else?** (WASM, Python, etc.) for ecosystem access
2. **How should tools be typed?** Static tool definitions vs dynamic discovery
3. **What's the debugging story?** Agent systems are notoriously hard to debug

## Resolved Questions

- **How do we handle agent failures?** ✓ Implemented via `try`/`catch`/`on error` (RFC-0007)
- **How do agents access external services?** ✓ Implemented via built-in tools with `use` declaration (RFC-0011)
- **How do we get editor support?** ✓ Implemented via `sage sense` LSP server with Zed and VS Code extensions (RFC-0014)

---

*Ward watches. Ward waits. The future will tell if the bet was right.*
