# Basic usage example

```bash
localize workspace create demo
localize project create demo-app --workspace demo --default-locale en-US
localize locale add es-PE --workspace demo --project demo-app
localize translation set welcome "Welcome, {name}!" --locale en-US --workspace demo --project demo-app
localize translation set welcome "¡Bienvenido!" --locale es-PE --workspace demo --project demo-app
localize check --workspace demo --project demo-app
localize export --workspace demo --project demo-app --out ./locales --nested
```
