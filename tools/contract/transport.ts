/** Test-only handlers on the pinned, unmodified upstream transport and codec.
 * Copied to the disposable upstream source root by xtask; never shipped.
 */
import { WsRpcServer } from './packages/server-core/src/transport/server'
import { EVENT_BUFFER_MAX_SIZE } from '@mkagent/shared/protocol'
const token = process.env.SHIPAGENT_CONTRACT_TOKEN
if (!token) throw new Error('Missing isolated contract token')
const server = new WsRpcServer({
  port: 0, host: '127.0.0.1', requireAuth: true,
  validateToken: async value => value === token,
  serverVersion: 'shipagent-contract-v0.2.6',
})
let writes = 0
let releases: (() => void)[] = []
server.handle('contract:info', () => ({ online: server.getConnectedClientCount(), writes, bufferLimit: EVENT_BUFFER_MAX_SIZE }))
server.handle('contract:echo', (_ctx, value) => value)
server.handle('contract:fail', () => { throw Object.assign(new Error('fixture failure'), { code: 'HANDLER_ERROR' }) })
server.handle('contract:push', (ctx, values: number[]) => {
  for (const value of values) server.push('contract:event', { to: 'workspace', workspaceId: ctx.workspaceId! }, value)
})
server.handle('contract:reverse', (ctx, channel: string, value) => server.invokeClient(ctx.clientId, channel, value))
server.handle('contract:slow', async (_ctx, ms: number, value) => {
  await Bun.sleep(ms)
  return value
})
server.handle('contract:commit', async () => {
  writes++
  await new Promise<void>(resolve => releases.push(resolve))
  return writes
})
server.handle('contract:releaseWrites', () => { const pending = releases; releases = []; for (const release of pending) release() })
await server.listen()
console.log(`CONTRACT_URL=ws://127.0.0.1:${server.port}`)
process.on('SIGTERM', () => { server.close(); process.exit(0) })
