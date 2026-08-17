# @alchetron/sdk

Zero-dependency JavaScript client for the versioned Alchetron HTTP API. Public reads need no token. Operator mutations require the bearer token printed by `algo ui`.

```js
import { AlchetronClient } from "@alchetron/sdk";
const alchetron = new AlchetronClient("http://127.0.0.1:8791");
console.log(await alchetron.graph());
```
