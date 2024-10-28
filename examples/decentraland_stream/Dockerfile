FROM node:18-alpine

RUN npm install --global "@warren-bank/hls-proxy"

EXPOSE 3001

RUN hlsd --port "3001" --host "0.0.0.0" --origin "https://player.twitch.tv" --referer "https://www.twitch.tv/" --useragent "Mozilla/5.0 (Linux; Android 6.0; Nexus 5 Build/MRA58N) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Mobile Safari/537.36" 

