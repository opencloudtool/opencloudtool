FROM python:3.12-alpine

WORKDIR /app

COPY pyproject.toml poetry.lock* /app/

RUN pip install poetry && \
    poetry config virtualenvs.create false && \
    poetry install --no-interaction --no-ansi

COPY . /app

CMD ["uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8000"]
