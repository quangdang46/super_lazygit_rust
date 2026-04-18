# run with:
# docker build -t slg .
# docker run -it slg:latest /bin/sh

FROM golang:1.25 as build
WORKDIR /go/src/github.com/jesseduffield/slg/
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 GOOS=linux go build

FROM alpine:3.19
RUN apk add --no-cache -U git xdg-utils
WORKDIR /go/src/github.com/jesseduffield/slg/
COPY --from=build /go/src/github.com/jesseduffield/slg ./
COPY --from=build /go/src/github.com/jesseduffield/slg/slg /bin/
RUN echo "alias gg=slg" >> ~/.profile

ENTRYPOINT [ "slg" ]
