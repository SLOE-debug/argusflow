import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const file = resolve(process.argv[2]);
const body = await readFile(file);
createServer((request, response) => {
  if (request.url !== "/" || !["GET", "HEAD"].includes(request.method)) {
    response.writeHead(404);
    response.end();
    return;
  }
  response.writeHead(200, {
    "Content-Type": "text/html; charset=utf-8",
    "Cache-Control": "no-store",
    "Content-Length": body.length,
  });
  response.end(request.method === "HEAD" ? undefined : body);
}).listen(0, "127.0.0.1", function () {
  console.log(`http://127.0.0.1:${this.address().port}/`);
});
