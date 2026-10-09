// Add-provider wizard, key test result and account states (keys must match pl-hub.ts).
export const enHub = {
  'hub.state.unconfigured': 'not configured',
  'hub.state.testing': 'testing…',
  'hub.state.ok': 'working',
  'hub.state.invalid': 'key rejected',
  'hub.state.rate_limited': 'rate limited',
  'hub.state.disabled': 'disabled',
  'hub.test': 'Test connection',
  'hub.toModels': 'Models and queues',
  'hub.rotate': 'Change key',
  'hub.rotateTitle': 'New key for “{label}”',
  'hub.rotateSubmit': 'Check and replace key',
  'hub.rotated': 'The new key was saved in place of the previous one.',
  'hub.rotateKept': 'The new key was not saved — the account still uses the previous one.',
  'hub.rotateOther': 'Paste another key',
  'hub.disable': 'Disable',
  'hub.enable': 'Enable',
  'hub.why': 'What to do',
  'hub.reason.ok': 'working',
  'hub.reason.restricted': 'working · restricted permissions',
  'hub.reason.rate_limited': 'rate limited',
  'hub.reason.invalid_key': 'key rejected',
  'hub.reason.no_credit': 'no credit on the account',
  'hub.reason.region': 'country not supported',
  'hub.reason.refused': 'access refused',
  'hub.reason.timeout': 'no answer',
  'hub.reason.network': 'connection error',
  'hub.reason.unsupported': 'not tested',

  'wiz.title': 'Add provider',
  'wiz.stepOf': 'Step {n} of {total}: {name}',
  'wiz.step.provider': 'Provider',
  'wiz.step.key': 'Key',
  'wiz.step.test': 'Test',
  'wiz.step.models': 'Models',
  'wiz.step.assign': 'Assignment',
  'wiz.step.limit': 'Limit',
  'wiz.filter': 'Filter providers',
  'wiz.label': 'Account name',
  'wiz.key': 'API key',
  'wiz.keyHint':
    'Paste the key. It is never written to files — it goes to Windows Credential Manager.',
  'wiz.baseUrl': 'Endpoint address (base URL)',
  'wiz.baseUrlHint':
    "This provider's API address from its documentation, starting with https://. The key is sent only to this address.",
  'wiz.keyFail': 'Key not saved: {error}',
  'wiz.testing': 'Checking the key with {name}…',
  'wiz.keyTitle': 'API key from {name}',
  'wiz.howTitle': 'Where to get a key',
  'wiz.how.open': 'Open the keys page in your browser and sign in:',
  'wiz.how.copy': 'Copy address',
  'wiz.how.copied': 'Copied',
  'wiz.how.create':
    'Create a new key (“Create new secret key” or “Create key”) and copy it right away — it cannot be shown again later.',
  'wiz.how.paste': 'Paste the key below (Ctrl+V).',
  'wiz.how.billing':
    'The API is a separate, paid {name} service: a chat subscription (e.g. ChatGPT Plus, Claude Pro) does not cover it. The key works only once the API account has credit or a card attached.',
  'wiz.showKey': 'Show key',
  'wiz.labelHint': 'Just for you — how the account is named in the list.',
  'wiz.checkKey': 'Check key',
  'wiz.check.ok': 'Looks like a key. I will check with the provider whether it works.',
  'wiz.check.notKey':
    'This does not look like an API key — it has spaces or non-Latin characters. Paste the key alone, without any description.',
  'wiz.check.cliToken':
    'This is a Claude Code sign-in token, not an API key. Create an API key on the page shown above.',
  'wiz.check.other':
    'This looks like a {other} key, but the selected provider is {name}. If that is a mistake, go back to the “Provider” step.',
  'wiz.check.prefix':
    '{name} keys usually start with “{prefix}”. Check that the whole key was copied.',
  'wiz.check.short': 'This is very short — API keys are longer. Check that all of it was copied.',
  'wiz.check.tidied':
    'I will remove clipboard extras (quotes, “NAME=”, spaces, line breaks) — only the key itself is saved.',
  'wiz.testAgain': 'Check again',
  'wiz.fixTitle': 'What to do',
  'wiz.detailTitle': 'What exactly {name} replied',
  'wiz.detail.status': 'HTTP status',
  'wiz.detail.code': 'Error code',
  'wiz.detail.message': 'Message',
  'wiz.verdict.ok': 'The key works',
  'wiz.verdict.restricted': 'The key works, but has restricted permissions',
  'wiz.verdict.rate_limited': 'The provider is rate limiting for a moment',
  'wiz.verdict.invalid_key': '{name} did not accept this key',
  'wiz.verdict.no_credit': 'The key is valid, but the account has no credit',
  'wiz.verdict.region': '{name} does not support this country or region',
  'wiz.verdict.refused': '{name} refused access',
  'wiz.verdict.timeout': '{name} did not answer in time',
  'wiz.verdict.network': 'Could not connect to {name}',
  'wiz.verdict.unsupported': 'This key cannot be checked up front',
  'wiz.why.restricted':
    'The key is valid but may not list models. Chat will work once you name the model Alfa should use.',
  'wiz.why.rate_limited':
    'The request limit was used up for a moment, so the provider has not checked the key yet. It does not mean the key is wrong.',
  'wiz.why.invalid_key':
    'The provider says this key does not exist, has expired or was deleted. Most often it is an incomplete copy or a key from another account.',
  'wiz.why.no_credit':
    'The provider recognised the key but refuses because the API account has no credit or active billing.',
  'wiz.why.region':
    'The provider blocks API access from this country or network. The key may be valid.',
  'wiz.why.refused':
    'The provider refused for a reason other than the key itself: the API is disabled on the account, the key has no access to the project, or a firewall. The key may be fine.',
  'wiz.why.timeout': 'The provider did not answer in time.',
  'wiz.why.network': "Connecting to the provider's server failed. The key may not be at fault.",
  'wiz.why.unsupported':
    'This provider or address offers no way to check a key before a chat. The key is saved — the first chat will show whether it works.',
  'wiz.fix.restricted.1':
    'Name the model (for example gpt-4o-mini) — exactly as the provider spells it.',
  'wiz.fix.restricted.2':
    'Or give the key permission to read models (“Models: Read”), or create a key with full permissions (“All”): {url}. Then check again.',
  'wiz.fix.rate_limited.1':
    'Wait a minute and check again. You can keep the account — if the key is good, it will start working by itself.',
  'wiz.fix.invalid_key.1': 'Create a new key ({url}) and copy it with the copy button — all of it.',
  'wiz.fix.invalid_key.2':
    'Check that the key belongs to the account and project you want to use and has not been deleted.',
  'wiz.fix.invalid_key.3': "Make sure it is a {name} key, not another provider's.",
  'wiz.fix.invalid_key.4': 'Paste the new key — the button is below.',
  'wiz.fix.no_credit.1':
    "Top up the API account or add a card in the provider's billing (separate from a chat subscription).",
  'wiz.fix.no_credit.2':
    'After topping up, check again — no new key is needed. The change can take a few minutes to show.',
  'wiz.fix.region.1':
    "Check the provider's list of supported countries; a VPN or company network may change the visible country.",
  'wiz.fix.refused.1':
    'Open “What exactly … replied” below — the provider usually states the reason there.',
  'wiz.fix.refused.2':
    'In your provider account settings ({url}) check that the API is enabled and the key belongs to the right project; then check again.',
  'wiz.fix.timeout.1': 'Check your Internet connection and check again.',
  'wiz.fix.timeout.2':
    "If it keeps happening, the provider's status page will say whether there is an outage.",
  'wiz.fix.network.1': 'Check your Internet connection, firewall and VPN, then check again.',
  'wiz.fix.network.2': 'With a custom endpoint address, check that it is typed correctly.',
  'wiz.fix.unsupported.1':
    'If there is no model list, name the model Alfa should use — exactly as the provider spells it.',
  'wiz.testOk': 'Connected · {latency}',
  'wiz.fixKey': 'Fix the key',
  'wiz.keepAccount': 'Keep the account and close',
  'wiz.finishLater': 'Close — I will set the rest later',
  'wiz.providerSite': 'the provider site',
  'wiz.modelName': 'Model name',
  'wiz.modelSave': 'Save model',
  'wiz.modelSet': 'Alfa will use the model “{model}”.',
  'wiz.modelsFound': { one: '{n} model found', other: '{n} models found' },
  'wiz.tasks': 'Task classes',
  'wiz.task.chat': 'Conversation',
  'wiz.task.code': 'Code',
  'wiz.task.planning': 'Planning',
  'wiz.task.summarize': 'Summarizing',
  'wiz.agents': 'Agents',
  'wiz.voice': 'Voice',
  'wiz.stt': 'Speech recognition (STT)',
  'wiz.tts': 'Speech synthesis (TTS)',
  'wiz.limitToggle': 'Cost limit for this provider',
  'wiz.limitAmount': 'Monthly amount (PLN)',
  'wiz.finish': 'Finish',
  'wiz.skip': 'Skip — I will add it later',
  'wiz.added': 'Account "{label}" added.',
} as const;
