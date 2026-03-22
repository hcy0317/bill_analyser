import asyncio
from src.api.app import initialize

asyncio.run(initialize())
print('APP_INIT_OK')
