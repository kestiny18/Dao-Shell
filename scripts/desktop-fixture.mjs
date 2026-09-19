// Local synthetic model for repeatable desktop checks. Never reads user configuration or credentials.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
const directory = path.resolve(process.argv[2] || '.tools/desktop-fixture');
const files = path.join(directory, 'files');
fs.mkdirSync(files, { recursive: true });
fs.writeFileSync(path.join(files, '合同-桌面验证.txt'), 'Dao-Shell synthetic desktop fixture.\n');
fs.writeFileSync(path.join(files, '合同-补充说明.txt'), 'Another synthetic document.\n');
const server = http.createServer((req, res) => {
  if (req.method !== 'POST' || req.url !== '/v1/chat/completions') { res.writeHead(404).end(); return; }
  let body = '';
  req.on('data', (chunk) => { body += chunk; if (body.length > 200000) req.destroy(); });
  req.on('end', () => {
    try {
      const { messages } = JSON.parse(body);
      const last = messages.at(-1);
      const input = messages.findLast((message) => message.role === 'user')?.content || '';
      const tool = (name, args) => ({ role: 'assistant', content: null, tool_calls: [{ id: `fixture-${Date.now()}`, type: 'function', function: { name, arguments: JSON.stringify(args) } }] });
      let response;
      if (last.role !== 'tool') response = tool('file_search', { query: input.includes('不存在') ? 'definitely-absent' : '合同', kind: 'file' });
      else {
        const result = JSON.parse(last.content);
        if (result.items && result.items.length && input.includes('打开')) response = tool('file_open', { object_id: result.items[0].id });
        else response = { role: 'assistant', content: result.items ? `本地测试模型：找到 ${result.items.length} 个真实候选，可以在下面选择。` : `本地测试模型：${result.message || result.error || '请求已结束。'}` };
      }
      const send = () => { if (!res.destroyed) { res.setHeader('Content-Type', 'application/json'); res.end(JSON.stringify({ choices: [{ message: response }] })); } };
      if (input.includes('慢') && last.role !== 'tool') setTimeout(send, 10000); else send();
    } catch { res.writeHead(400).end(); }
  });
});
server.listen(0, '127.0.0.1', () => {
  const config = path.join(directory, 'config.json');
  fs.writeFileSync(config, JSON.stringify({ read_roots: [files], write_roots: [], model: { endpoint: `http://127.0.0.1:${server.address().port}/v1/chat/completions`, model: '本地测试模型（固定响应）', api_key_env: '' } }, null, 2));
  console.log(JSON.stringify({ config, files, port: server.address().port }));
});
