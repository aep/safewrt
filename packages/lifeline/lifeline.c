#include <sys/socket.h>
#include <sys/types.h>
#include <netinet/tcp.h>
#include <netinet/in.h>
#include <netdb.h>
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <unistd.h>
#include <errno.h>
#include <arpa/inet.h>
#include <stropts.h>
#include <poll.h>
#include "config.h"
#include <sys/un.h>
#include <sys/socket.h>
#include <unistd.h>
#include <fcntl.h>
#include <poll.h>

char *build_helo(const char *remotename)
{
    static char helo[2048];
    char name[200];
    if (gethostname(name, 200) != 0) {
        name[0] = '?';
        name[1] = 0;
    }
    snprintf(helo, 2048,
            "GET /lifeline/1 HTTP/1.1\r\n"
            "Host: %s\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            "User-Agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/51.0.2704.63 Safari/537.36\r\n"
            "Sec-WebSocket-Key: x3JJHMbDL1EzLkh9GBhXDw==\r\n"
            "Sec-WebSocket-Protocol: chat, superchat\r\n"
            "Sec-WebSocket-Version: 13\r\n"
            "X-LF-Name: legacy.%s\r\n"
            "\r\n",
            remotename,
            name);

    return helo;
}

int remote_connect(int address)
{
    int sockfd = 0, n = 0;
    struct sockaddr_in serv_addr;

    if((sockfd = socket(AF_INET, SOCK_STREAM, 0)) < 0) {
        printf("Error : Could not create socket \n");
        return -1;
    }

    memset(&serv_addr, '0', sizeof(serv_addr));

    serv_addr.sin_family = AF_INET;
    serv_addr.sin_port = htons(80);
    serv_addr.sin_addr.s_addr = address;

    fcntl(sockfd, F_SETFL, fcntl(sockfd, F_GETFL, 0) | O_NONBLOCK);


    connect(sockfd, (struct sockaddr *)&serv_addr, sizeof(serv_addr));

    struct pollfd fds[1];
    fds[0].fd = sockfd;
    fds[0].events = POLLOUT | POLLIN;
    int r = poll(fds, 1, 5000);
    if (r != 1) {
        //timeout
        printf("Error : Timeout\n");
        close(sockfd);
        return -3;
    }
    fcntl(sockfd, F_SETFL, fcntl(sockfd, F_GETFL, 0) & ~O_NONBLOCK);


    int optval = 1;
    socklen_t optlen = sizeof(optval);
    if (setsockopt(sockfd, SOL_SOCKET, SO_KEEPALIVE, &optval, optlen) != 0) {
        perror("SO_KEEPALIVE");
    }

    optval = 10;
    optlen = sizeof(optval);
    if (setsockopt(sockfd, IPPROTO_TCP, TCP_KEEPIDLE, &optval, optlen) != 0) {
        perror("TCP_KEEPIDLE");
    }

    optval = 10;
    optlen = sizeof(optval);
    if (setsockopt(sockfd, IPPROTO_TCP, TCP_KEEPINTVL, &optval, optlen) != 0) {
        perror("TCP_KEEPINTVL");
    }

    optval = 10;
    optlen = sizeof(optval);
    if (setsockopt(sockfd, IPPROTO_TCP, TCP_KEEPCNT, &optval, optlen) != 0) {
        perror("TCP_KEEPCNT");
    }

    return sockfd;
}

int local_connect()
{
    printf("connecting to localhost:22\n");
    int sockfd = 0, n = 0;
    struct sockaddr_in serv_addr;

    if((sockfd = socket(AF_INET, SOCK_STREAM, 0)) < 0)
    {
        printf("Error : Could not create socket \n");
        return -1;
    }

    memset(&serv_addr, '0', sizeof(serv_addr));

    serv_addr.sin_family = AF_INET;
    serv_addr.sin_port = htons(22);

    if(inet_pton(AF_INET, "127.0.0.1", &serv_addr.sin_addr)<=0)
    {
        printf("inet_pton error occured\n");
        return -1;
    }

    if( connect(sockfd, (struct sockaddr *)&serv_addr, sizeof(serv_addr)) < 0)
    {
        printf("Error : Connect Failed \n");
        return -1;
    }

    return sockfd;
}

int splice(int fd_in, int _1, int fd_out, int _2, int _3, int _4)
{
    static char buf[1024];
    int size = read(fd_in, buf, 1024);
    if (size < 1) {
        return size;
    }
    return write(fd_out, buf, size);
}

void forward_loop(int fd1, int fd2)
{
    for (;;) {
        struct pollfd fds[2];
        fds[0].fd = fd1;
        fds[1].fd = fd2;
        fds[0].events = POLLIN;
        fds[1].events = POLLIN;
        int ret = poll(fds, 2, -1);
        if (ret < 1) {
            perror("poll");
            exit(ret);
        }

        if (fds[0].revents) {
            if (splice(fd1, 0, fd2, 0, 1024,0) < 1) {
                return;
            }

        }
        if (fds[1].revents) {
            if (splice(fd2, 0, fd1, 0, 1024,0) < 1) {
                return;
            }
        }
    }
}

int attempt_remote(const char *connectName, int address)
{
    int remote_socket  = remote_connect(address);
    if (remote_socket > 1) {
        printf("connected %u\n", remote_socket);
        //send client helo
        char *helo = build_helo(connectName);
        send(remote_socket, helo, strlen(helo), 0);

        //server helo
        //expect upgrade
        const char *expect = "HTTP/1.1 101 Switching Protocols";
        int expect_len = strlen(expect);
        char buf[expect_len];
        if (recv(remote_socket, buf, expect_len, 0) != expect_len) {
            printf("socket died in server helo 1\n");
            close(remote_socket);
            return 1;
        }
        if (strncmp(expect, buf, expect_len) != 0) {
            printf("invalid server helo 1\n");
            close(remote_socket);
            return 2;
        }


        //ignore all the headers
        //so the server can send us some spam for firewall piercing and shit
        int countspace = 0;
        for (;;) {
            //TODO this is inefficient as fuck. but we don't want to eat ssh frames
            if (recv(remote_socket, helo, 1,0) != 1) {
                printf("socket died in server helo 2\n");
                close(remote_socket);
                return 1;
            }
            if (helo[0] == '\n' || helo[0] == '\r') {
                if (++countspace > 3){
                    printf("server helo completed\n");
                    break;
                }
            } else {
                countspace = 0;
            }

        }

        int local_socket  = local_connect();
        if (local_socket < 1) {
            return local_socket;
        }

        forward_loop(local_socket, remote_socket);
        return 0;
    }
}

#include "dns.c"

int attempt_dns(const char *dns_ip)
{
    fprintf(stderr, "trying dns: %s\n", dns_ip);
    for (char **e = LIFELINE_SERVERS; *e != 0; e++) {
        printf("connecting to: %s:80\n", *e);
        int address = inet_addr(*e);
        if (address == INADDR_NONE) {
            dns_t dns;
            if (dns_init(&dns, dns_ip)) {
                fprintf(stderr, "dns fail: init\n");
                continue;
            }
            dns_set_timeout(&dns, 5);
            if (dns_request(&dns, *e)) {
                fprintf(stderr, "dns fail: init\n");
                dns_close(&dns);
                continue;
            }
            if (dns_receive(&dns)) {
                fprintf(stderr, "dns fail: receive\n");
                dns_close(&dns);
                continue;
            }
            for(dns_record_t *r = dns.records; r->ttl != 0; r++) {
                attempt_remote(*e, r->address);
            }
            dns_close(&dns);
        } else {
            attempt_remote(*e, address);
        }
    }
}

void parse_resolvconf(const char *filename) {

    FILE *fp = fopen(filename, "r");
    if (!fp) {
        return;
    }

    char buf[1024];
    for(;;){
        const char *line = fgets((char*)buf, 1024, fp);
        if (line == 0) {
            break;
        }
        if (sscanf(line, "nameserver %s", &buf) == 1) {
            attempt_dns(buf);
        }
    }
    fclose(fp);
}

int main(int argc, char *argv[])
{
    for (char **re = DNS_RESOLVERS; *re != 0; re++){
        if (*re[0] == '/') {
            parse_resolvconf(*re);
        } else {
            attempt_dns(*re);
        }
    }
    fprintf(stderr, "\n\n");
    sleep(5);
    //we're leaking sockets somewhere, lets just exit
}
