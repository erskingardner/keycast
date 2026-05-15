import { existsSync, readFileSync, writeFileSync } from "node:fs";

const handlerPath = new URL("../build/handler.js", import.meta.url);

if (!existsSync(handlerPath)) {
    throw new Error("Expected build/handler.js after vite build");
}

const source = readFileSync(handlerPath, "utf8");
const before = "const handleWebsocket = server.websocket();";
const after =
    'const handleWebsocket = typeof server.websocket === "function" ? server.websocket() : null;';

if (!source.includes(before) && !source.includes(after)) {
    throw new Error("Could not find svelte-adapter-bun websocket probe in build/handler.js");
}

if (source.includes(before)) {
    writeFileSync(handlerPath, source.replace(before, after), "utf8");
}
