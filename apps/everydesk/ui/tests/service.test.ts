import assert from 'node:assert/strict'
import { test } from 'node:test'
import { serviceError, serviceView, UNSUPPORTED_HINT, type ServiceStatus } from '../src/lib/home/service.ts'

const win = (mode: ServiceStatus['mode']): ServiceStatus => ({ mode, platform_supported: true })

test('portable on Windows: offers the install, explains the one-time admin prompt', () => {
  const v = serviceView(win('portable'))
  assert.equal(v.status, 'Portátil: funciona enquanto o app está aberto.')
  assert.equal(v.action?.label, 'Instalar para acesso permanente')
  assert.equal(v.action?.command, 'install_permanent')
  assert.match(v.hint, /administrador uma única vez/)
  assert.equal(v.tone, 'off')
  assert.equal(v.disabled, false)
})

test('installed: says so and offers the removal', () => {
  const v = serviceView(win('installed'))
  assert.equal(v.status, 'Instalado: inicia com o Windows e funciona na tela de bloqueio.')
  assert.equal(v.action?.label, 'Remover acesso permanente')
  assert.equal(v.action?.command, 'uninstall_permanent')
  assert.equal(v.tone, 'on')
})

test('service stopped: warns and offers a restart', () => {
  const v = serviceView(win('installed-stopped'))
  assert.equal(v.status, 'Serviço parado.')
  assert.equal(v.action?.label, 'Reiniciar serviço')
  assert.equal(v.action?.command, 'install_permanent')
  assert.equal(v.tone, 'warn')
  assert.equal(v.disabled, false)
})

test('macOS/Linux: only on Windows, button disabled', () => {
  const v = serviceView({ mode: 'portable', platform_supported: false })
  assert.equal(v.hint, UNSUPPORTED_HINT)
  assert.equal(v.hint, 'Disponível no Windows nesta versão.')
  assert.equal(v.disabled, true)
  assert.equal(v.action?.label, 'Instalar para acesso permanente')
})

test('before the first status: nothing to click', () => {
  const v = serviceView(null)
  assert.equal(v.action, null)
  assert.equal(v.disabled, true)
})

test('errors read as plain text', () => {
  assert.equal(serviceError('A permissão de administrador não foi concedida.'), 'A permissão de administrador não foi concedida.')
  assert.equal(serviceError(new Error('x')), 'x')
  assert.equal(serviceError(''), 'Não consegui concluir. Tente de novo.')
})
