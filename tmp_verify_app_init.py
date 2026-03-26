import asyncio

from bill_analyser.api.app import initialize

asyncio.run(initialize())
print("APP_INIT_OK")
