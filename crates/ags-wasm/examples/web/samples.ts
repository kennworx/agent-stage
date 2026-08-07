// One sample per diagram type the engine draws — lifted from
// `examples/diagram-gallery.md`, so every one is a source the renderer is already
// tested against rather than something written for a screenshot.
//
// Generated. Regenerate after changing the gallery; the page checks this list
// against `diagram_kinds()` and says so when a type has no sample here.

export const DIAGRAMS: ReadonlyArray<readonly [string, string]> = [
  ["architecture", "architecture-beta\n    group net(internet)[Network]\n    service gateway(server)[Gateway] in net\n    service api1(server)[API 1] in net\n    service api2(server)[API 2] in net\n    junction j in net\n    gateway:B -- T:j\n    j:L -- T:api1\n    j:R -- T:api2"],
  ["block", "block-beta\n    columns 4\n    Client[\"Client\"] space space space\n    API[\"API Gateway\"] Auth[\"Auth\"] Cache[\"Cache\"] Queue[\"Queue\"]\n    DB[\"Database\"] space space Worker[\"Worker\"]\n    Client --> API\n    API --> Auth\n    API --> Cache\n    API --> DB\n    Queue --> Worker"],
  ["C4Context", "C4Context\n    Person(user, \"User\", \"An end user\")\n    System_Boundary(app, \"Web App\") {\n      Container(spa, \"SPA\", \"React\", \"The UI\")\n      Container(api, \"API\", \"Node\", \"Business logic\")\n      ContainerDb(db, \"Database\", \"Postgres\", \"Stores data\")\n    }\n    Rel(user, spa, \"Uses\")\n    Rel(spa, api, \"Calls\", \"JSON/HTTP\")\n    Rel(api, db, \"Reads/Writes\")"],
  ["classDiagram", "classDiagram\n  class Status {\n    <<enumeration>>\n    ACTIVE\n    INACTIVE\n    PENDING\n    DELETED\n  }"],
  ["erDiagram", "erDiagram\n  ORDER {\n    int id PK\n    int customer_id FK\n    string invoice_number UK\n    decimal total\n    date order_date\n    string status\n  }"],
  ["eventmodeling", "eventmodeling\n    title Order Placement\n    tf 01 ui OrderForm\n    tf 02 cmd PlaceOrder\n    tf 03 evt OrderPlaced\n    tf 04 pcr ChargeCard\n    tf 05 evt PaymentTaken\n    tf 06 rmo OrderStatus"],
  ["flowchart", "graph TD\n  A[Start] --> B{Decision}\n  B -->|Yes| C[Accept]\n  B -->|No| D[Reject]\n  C --> E[Done]\n  D --> E\n  linkStyle 0 stroke:#7aa2f7,stroke-width:3px\n  linkStyle 1 stroke:#9ece6a,stroke-width:2px\n  linkStyle 2 stroke:#f7768e,stroke-width:2px\n  linkStyle default stroke:#565f89"],
  ["gantt", "gantt\n    title Roadmap\n    dateFormat YYYY-MM-DD\n    section Design\n    Research : r1, 2024-01-01, 7d\n    Spec : after r1, 5d\n    section Build\n    Parser : after r1, 10d"],
  ["gitGraph", "gitGraph\n    commit id: \"init\"\n    commit id: \"setup\" tag: \"v0.1\"\n    branch feature\n    commit id: \"ui\"\n    commit id: \"logic\"\n    checkout main\n    commit id: \"hotfix\"\n    merge feature tag: \"v1.0\""],
  ["ishikawa", "ishikawa\n    Late Delivery\n        Process\n            Slow approvals\n            Manual steps\n        People\n            Understaffed"],
  ["journey", "journey\n    title Online Shopping\n    section Browse\n      Search products : 4: Customer\n      Read reviews : 3: Customer\n    section Checkout\n      Add to cart : 5: Customer\n      Enter payment : 2: Customer, System\n      Confirm order : 5: Customer"],
  ["kanban", "kanban\n    todo[To Do]\n        t1[Design spec]\n        t2[Write tests]\n    doing[In Progress]\n        t3[Build parser]\n    done[Done]\n        t4[Setup CI]"],
  ["mindmap", "mindmap\n  root((Beautiful Mermaid))\n    Rendering\n      SVG\n      ASCII\n    Themes\n      Dark\n      Light"],
  ["packet", "packet\n    0-15: \"Source Port\"\n    16-31: \"Destination Port\"\n    32-63: \"Sequence Number\""],
  ["pie", "pie title Browser Market Share\n    \"Chrome\" : 64\n    \"Safari\" : 19\n    \"Edge\" : 5\n    \"Firefox\" : 3\n    \"Other\" : 9"],
  ["quadrantChart", "quadrantChart\n    title Reach vs Engagement\n    x-axis Low Reach --> High Reach\n    y-axis Low Engagement --> High Engagement\n    quadrant-1 Expand\n    quadrant-2 Promote\n    quadrant-3 Re-evaluate\n    quadrant-4 Improve\n    Campaign A: [0.3, 0.6]\n    Campaign B: [0.45, 0.23]\n    Campaign C: [0.57, 0.69]"],
  ["radar", "radar-beta\n    title Product Comparison\n    axis price[\"Price\"], perf[\"Performance\"], ux[\"UX\"], support[\"Support\"], docs[\"Docs\"]\n    curve us[\"Us\"]{4, 5, 4, 3, 5}\n    curve them[\"Competitor\"]{5, 3, 2, 4, 2}"],
  ["requirementDiagram", "requirementDiagram\n    requirement speed {\n      id: 1\n      text: render under 50ms\n      risk: high\n      verifymethod: test\n    }\n    functionalRequirement themes {\n      id: 2\n      text: support theming\n    }\n    element renderer {\n      type: module\n    }\n    element suite {\n      type: tests\n    }\n    renderer - satisfies -> speed\n    renderer - satisfies -> themes\n    suite - verifies -> speed"],
  ["sankey", "sankey\n    Solar,Grid,30\n    Wind,Grid,45\n    Coal,Grid,25\n    Grid,Residential,50\n    Grid,Commercial,30\n    Grid,Industrial,20"],
  ["sequenceDiagram", "sequenceDiagram\n  participant C as Client\n  participant S as Server\n  C->>S: Connect\n  loop Every 30s\n    C->>S: Heartbeat\n    S-->>C: Ack\n  end\n  C->>S: Disconnect"],
  ["timeline", "timeline\n    title History of the Web\n    1991 : First website\n    2004 : Gmail : Facebook\n    2008 : Chrome : GitHub\n    2015 : ES6 : React Native"],
  ["treemap", "treemap\n  \"Repo\"\n    \"src\"\n      \"renderers\": 60\n      \"parsers\": 45\n      \"layout\": 30\n    \"tests\": 40\n    \"docs\": 15"],
  ["treeview", "treeView-beta\n  \"src/\"\n    \"index.ts\"\n    \"parser.ts\"\n  \"docs/\"\n    \"guide.md\""],
  ["venn", "venn-beta\n    title Skills\n    set Design\n    set Code\n    set Product\n    union Design, Code\n    union Code, Product"],
  ["wardley", "wardley\n    title Tea Shop\n    anchor Business [0.95, 0.55]\n    component Cup [0.84, 0.30]\n    component Tea [0.70, 0.55]\n    component Hot_Water [0.60, 0.70]\n    component Kettle [0.45, 0.78]\n    component Power [0.20, 0.92]\n    Business -> Cup\n    Cup -> Tea\n    Tea -> Hot_Water\n    Hot_Water -> Kettle\n    Kettle -> Power"],
  ["xychart", "xychart-beta\n    title \"Monthly Revenue\"\n    x-axis \"Month\" [Jan, Feb, Mar, Apr, May, Jun, Jul, Aug, Sep, Oct, Nov, Dec]\n    y-axis \"Revenue (USD)\" 0 --> 10000\n    bar [4200, 5000, 5800, 6200, 5500, 7000, 7800, 7200, 8400, 8100, 9000, 9200]\n    line [4200, 5000, 5800, 6200, 5500, 7000, 7800, 7200, 8400, 8100, 9000, 9200]"],
  ["zenuml", "zenuml\n    @Actor User\n    participant Server\n    loop (3 times) {\n      User->Server: poll\n      alt (ready) {\n        Server->User: data\n      } else {\n        Server->User: wait\n      }\n    }"],
];
