# Relatório do projeto de Administração de Sistemas

Como já estava a pensar aprender a usar o módulo [`optparse`][optparse] ou
[`argparse`][argparse] do Python em outro projeto, então neste decidi que ia
testar a criação de uma CLI em Rust, que já ouvi falar ser uma ótima experiência
graças à biblioteca [`clap`][clap].

[optparse]: <https://docs.python.org/3/library/optparse.html#module-optparse>
[argparse]: <https://docs.python.org/3/library/argparse.html#module-argparse>
[clap]: <https://docs.rs/clap/latest/clap/>

Criei logo o projeto:

```console
cargo new sysadmin
cd sysadmin
cargo add clap --features derive
```

E copiei exemplo do uso da biblioteca [disponível na sua documentação][example].

[example]: <https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html#quick-start>

Pelo exemplo fiquei logo contente que consigo criar _subcommands_ assim
facilmente, porque a minha ideia era criar uma função e _subcommand_ por alínea
do enunciado.
