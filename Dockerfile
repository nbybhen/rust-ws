FROM homebrew/brew

WORKDIR /usr/rust-ws

COPY . .

RUN sudo chmod -R 777 *

# Installs each required language through homebrew
RUN brew install rust
RUN brew install python
RUN brew install node
RUN brew install kotlin
RUN brew install llvm # Installs clang/clang++
RUN brew install typescript
RUN brew install go
RUN brew install elixir
RUN brew install java

EXPOSE 4000

CMD ["cargo", "run"]