# Under review

The three diagrams still being worked on, on their own so a change to one is
visible without scrolling past a hundred that did not change.

## git-branching-workflow

`approved` dips 43px below both of its own endpoints. The dip is the row
placement gave its dummy chain, not the lane. **Open — task #12.**

```mermaid #git-branching-workflow
graph LR
  A[main] --> B[develop]
  B --> C[feature/auth]
  B --> D[feature/ui]
  C --> E{PR Review}
  D --> E
  E -->|approved| B
  B --> F[release/1.0]
  F --> G{Tests?}
  G -->|pass| A
  G -->|fail| F
```

## state-connection-lifecycle

`error`/`success` no longer cross. But `close`, `done` and `max_retries`
each bend more times than the shape needs. **Open — task #13.**

```mermaid #state-connection-lifecycle
stateDiagram-v2
  [*] --> Closed
  Closed --> Connecting : connect
  Connecting --> Connected : success
  Connecting --> Closed : timeout
  Connected --> Disconnecting : close
  Connected --> Reconnecting : error
  Reconnecting --> Connected : success
  Reconnecting --> Closed : max_retries
  Disconnecting --> Closed : done
  Closed --> [*]
```

## requirement-verification

`«satisfies»` and `«verifies»` printed on top of each other; they now sit on
their own lines. **Fixed.**

```mermaid #requirement-verification
requirementDiagram
    requirement speed {
      id: 1
      text: render under 50ms
      risk: high
      verifymethod: test
    }
    functionalRequirement themes {
      id: 2
      text: support theming
    }
    element renderer {
      type: module
    }
    element suite {
      type: tests
    }
    renderer - satisfies -> speed
    renderer - satisfies -> themes
    suite - verifies -> speed
```

