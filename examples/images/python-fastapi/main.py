import os

from fastapi import FastAPI

app = FastAPI()


@app.get("/")
async def root():
    return {
        "message": "Hello World",
        "app_name": os.getenv("APP_NAME"),
    }
