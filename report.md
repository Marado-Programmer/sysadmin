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

# DNS e além

Reparei que muitas das alíneas do enunciado dependem da primeira: «permitir
receber um nome de domínio introduzido pelo utilizador e criar a zona master do
respectivo domínio de forma automática. Deverá pelo menos conter o registo IN A
para o IP respectivo do servidor de DNS/Web.»

Para começar, daqui para a frente vou assumir que estamos a usar uma
distribuição Linux com `dnf`, especificamente AlmaLinux 8.

Instalamos os pacotes necessários com `dnf install whois bind bind-utils`.
Depois no ficheiro `/etc/named.conf` precisamos verificar estas duas linhas:

```
listen-on port 53 {127.0.0.1; any;};
Allow-query {localhost; any;}
```

O que nós queremos é o `any;` dentro das chavetas, se já estiver lá, bom, senão,
teremos que adicionar. Depois no fim do ficheiro, adicionamos:

```
zone "<domain>" IN {
    type master;
    file "/var/named/<domain>.hosts";
}
```

E criamos o ficheiro `/var/named/<domain>.hosts` que terá o que o professor
pede, algo como:

```
$ttl 38400
@   IN  SOA dns.estig.pt.   mail.as.com.    (
                1165190726 ; serial
                10800 ; refresh
                3600 ; retry
                604800 ; expire
                38400 ; minimum
                )
    IN  NS  dns.estig.pt.
    IN  A   10.2.0.1
www IN  A   10.2.0.1
ftp IN  A   10.2.0.2
```

Atenção que isto requer previlégios de super utilizador para a maior parte das
tarefas.

Depois das modificações, é preciso usar uns estes comandos também:

```
systemctl start named
systemctl enable named
```

Talvez seja necessário também configurar o firewall e/ou o SELinux também, mas
isto já está fora do escopo deste projeto.

Isto foi a alínea 1. A alínea 3 pede para «criar a o VirtualHost de forma
automática, e ficar a responder automaticamente para o domínio criado por http.
Deverá ser criado automaticamente uma página de boas-vindas do respetivo
domínio.» Então:

```
sudo dnf install httpd -y
sudo systemctl enable --now httpd
```

Para depois no ficheiro `/etc/httpd/conf/httpd.conf` termos algo como:

```
NameVirtualHost 192.168.0.1:80
<VirtualHost 192.168.0.1:80>
DocumentRoot "/home/domain.com/"
ServerName www.domain.com
ServerAlias domain.com
<Directory "/home/domain.com">
    Options Indexes FollowSymLinks
    AllowOverride All
    Order allow,deny
    Allow from all
    Require method GET POST OPTIONS
</Directory>
</VirtualHost>
```

Na realidade, isto pode ficar num ficheiro separado reservado ao domínio
específicado na pasta `/etc/httpd/conf.d/`.

E se configurarmos com o resultado da alínea 1 o servidor DNS `named`, podemos
dizer para redirecionar para o IP do servidor HTTP.

O ficheiro `index.html` base estará em `/var/www/html/<domain>/`

Finalmente, é preciso `sudo systemctl restart httpd`, e talvez fazer algumas
mudanças em relação à firewall.

A alínea 4 pede para «permitir a criação de pelo menos registos do tipo A e MX
no servidor DNS de forma automática. O utilizador escolhe o domínio e o registo
a introduzir.»

O professor mostrou em aula que existem na realidade mais tipos de registo DNS:

| Tipo  | Descrição                                                                                               |
| ----- | ------------------------------------------------------------------------------------------------------- |
| A     | IPv4 Host Address 32-bit IP address                                                                     |
| AAAA  | IPv6 Host Address 128-bit IP address                                                                    |
| CNAME | Canonical Name Canonical Domain Name for an alias                                                       |
| MX    | Mail Exchanger 16-bit preference and name of host that acts as mail exchanger for the domain            |
| NS    | Name Server Name of authoritative server for domain                                                     |
| PTR   | Pointer Domain name (like a symbolic link)                                                              |
| SOA   | Start of Authority Multiple fields that specify which parts of the naming hierarchy a server implements |

Pertendo permitir que todos estes possam ser criados.

Basta reutilizar o suposto ficheiro `/var/named/<domain>.hosts` já existente e
adicionar uma linha a ele.

A alínea 5 pede para «permitir a criação de zonas reverse, o utilizador introduz
o IP e nome FQDN e é criado automaticamente a zona reverse se esta ainda não
existir.» Reutilizamos a criação de zonas DNS normais e a criação de registos
para zonas para esta alínea. E a alínea 6, «permitir a eliminação de zonas
master(forward), VirtualHosts e zonas reverse.», que é feito a partir a
eliminação de ficheiros e linhas de ficheiros.

A alínea 8 diz: «Além do ponto 1, qualquer melhoria no funcionamento do
scritp/aplicação, nomeadamente no que consiste em atualizações de registos de
DNS (forward ou reverse) e automatizações nos VirtualHosts, ou outras melhorias
no funcionamento da aplicação. Qualquer inovação será contemplada aquando da
avaliação.»

Só isto e este relatório, se estiverem perfeitos, seria 10 pontos, técnicamente
o mínimo para uma positiva. Sobrando fazer as alíneas 2, 7, e 9–12, porque a
alínea 13 tem relações com as anteriores:

## Blacklist

Uma solução, e a primeira que me tinha surgido, era adicionar linhas (e depois
removê-las) ao `/etc/hosts` a redirecionar para `0.0.0.0` ou similar. Mas dá
para reutilizar o código DNS acima e que penso estar mais correto por usar o
servidor DNS. Basicamente criamos uma master zone para o domínio que queremos
adicionar à blacklist, e depois adicionamos um registo `*` do tipo `A` para
`0.0.0.0`. O código de cima já faz isto tudo.

Mais 2 pontos.

# NFS

> permitir a configuração do serviço NFS, permitir a criação de partilhas do
> sistema de ficheiros no Linux no ficheiro “/etc/exports” para máquinas
> Linux/Unix. Deverá: Criar partilha, eliminar partilha, alterar partilha,
> desativar partilha. Deverá testar numa máquina Linux à parte o mapeamento NFS
> criado através do comando “mount –t nfs ….”.

É preciso fazer `dnf install nfs-utils net-tools` antes de tudo.

A tarefa baseia-se em ler o ficheiro "/etc/exports" e editar cada linha conforme
a necessidade.

Sem muito segredo.

Na máquina cliente depois, é necessário fazer
`mount -t nfs <serverip>:/<nfs> <mountpoint>`.

# Backups

> Efetuar os backups de ficheiros e configurações cruciais ao sistema (ficheiros
> das contas dos utilizadores e dos grupos), pode ser utilizado o utilitário
> “tar”.
>
> Utilizar o “rsync” para efetuar backups “incremental for ever” das áreas dos
> utilizadores do sistema.

Não tem muito segredo. Pelo menos a primeira parte onde basta criar um tarball
assim: `tar -czf backup.tar.gz /etc/passwd /etc/group /etc/shadow
/etc/gshadow`.

Agora para a parte do `rsync`, time que pesquisar sobre o "incremental for
ever". Basicamente, após o primeiro uso do `rsync` para criação do backup, os
próximos usos podem servir apenas para transferir apenas os ficheiros
modificados. Isto é feito usando `--link-dest=<lastbackup>` como uma option do
rsync após o primeiro backup.

# RAID

> Criar um raid nível 5 para segurança no armazenamento da informação. Deverá
> introduzir o nome da diretoria a montar a nova drive.

Com `mdadm` instalado (`dnf install mdadm`), faz:

```
mdadm --create --verbose /dev/md0 --level=5 \
    --raid-devices=3 /dev/vdb /dev/vdc /dev/vdd \
    --spare-devices=1 /dev/vde
```

Criar um filesystem para `/dev/md0`, e montá-lo.
