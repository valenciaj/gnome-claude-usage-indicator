# claude-usage-indicator

Indicador para la barra superior de GNOME, escrito en Rust, que muestra el uso de tu plan de
Claude (Pro/Max): el límite de sesión de 5 horas y el límite semanal.

- **Icono**: un donut doble. El anillo exterior es la sesión de 5 horas y el interior el
  límite semanal; se rellenan desde las 12 en punto en sentido horario. Verde por debajo del
  70 %, ámbar hasta el 90 % y rojo a partir de ahí. Si no hay datos, los anillos se ven grises.
- **Menú**: cada límite con su propio mini-donut, cuándo se reinicia, la hora de la última
  consulta, "Actualizar ahora", un enlace a `claude.ai/settings/usage` y "Salir".

## Requisitos

- GNOME con la extensión AppIndicator: `ubuntu-appindicators@ubuntu.com` (viene con Ubuntu)
  o [AppIndicator Support](https://extensions.gnome.org/extension/615/appindicator-support/).
- Haber iniciado sesión en **Claude Code** (`claude`), porque el indicador usa su token OAuth
  de `~/.claude/.credentials.json` (o de `$CLAUDE_CONFIG_DIR`).

## Instalación

```bash
./install.sh
```

Compila en modo release, copia el binario a `~/.local/bin` y añade una entrada de
autoarranque en `~/.config/autostart`. Para desinstalarlo:

```bash
pkill -x claude-usage-in
rm ~/.local/bin/claude-usage-indicator ~/.config/autostart/claude-usage-indicator.desktop
```

## Configuración

| Variable                | Por defecto   | Descripción                              |
|-------------------------|---------------|------------------------------------------|
| `CLAUDE_USAGE_INTERVAL` | `300`         | Segundos entre consultas (mínimo 60).    |
| `CLAUDE_CONFIG_DIR`     | `~/.claude`   | Carpeta con `.credentials.json`.         |

## Cómo funciona

Consulta `GET https://api.anthropic.com/api/oauth/usage` con la cabecera
`anthropic-beta: oauth-2025-04-20`, que es el mismo endpoint que usa `/usage` en Claude Code.
**No es una API pública documentada** y puede cambiar sin aviso.

El indicador no renueva el token por su cuenta, para no invalidar el de Claude Code. Si el token
caduca y Claude Code lleva tiempo cerrado, el menú lo indica; en cuanto abras Claude Code se
renueva y el indicador vuelve a mostrar los datos en la siguiente consulta.
