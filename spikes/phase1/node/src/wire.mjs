export function negotiate(minA, maxA, minB, maxB) {
  const min = Math.max(minA, minB);
  const max = Math.min(maxA, maxB);
  return min <= max ? max : null;
}

export function attachJsonLines(socket, onMessage, onError) {
  let buffer = "";
  socket.setEncoding("utf8");
  socket.on("data", chunk => {
    buffer += chunk;
    while (true) {
      const newline = buffer.indexOf("\n");
      if (newline < 0) break;
      const line = buffer.slice(0, newline).trim();
      buffer = buffer.slice(newline + 1);
      if (!line) continue;
      try { onMessage(JSON.parse(line)); }
      catch (error) { onError?.(error); }
    }
  });
}

export function sendJson(socket, value) {
  socket.write(`${JSON.stringify(value)}\n`);
}
