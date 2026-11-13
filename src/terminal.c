#define _DEFAULT_SOURCE
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stddef.h>
#include <sys/ioctl.h>
#include <termios.h>
#include <unistd.h>

/* Keep platform-specific POSIX structures out of the Rust FFI. */
static struct termios saved;
static int active;
static int output_fd = -1;
static volatile sig_atomic_t interrupted;
static const int signals[] = {SIGINT, SIGTERM, SIGHUP, SIGQUIT, SIGTSTP};
static struct sigaction previous[5];
static size_t registered;

static void stop(int signal) { (void)signal; interrupted = 1; }

void gm_terminal_restore(void) {
    if (active) {
        while (tcsetattr(STDIN_FILENO, TCSANOW, &saved) < 0 && errno == EINTR) {}
        active = 0;
    }
}

void gm_terminal_close(void) {
    gm_terminal_restore();
    if (output_fd >= 0) {
        close(output_fd);
        output_fd = -1;
    }
    while (registered) {
        --registered;
        sigaction(signals[registered], &previous[registered], NULL);
    }
}

int gm_terminal_open(void) {
    if (tcgetattr(STDIN_FILENO, &saved) < 0) return -1;
    /* Reopen instead of dup: O_NONBLOCK must not affect the shell's descriptors. */
    output_fd = open("/proc/self/fd/1", O_WRONLY | O_NONBLOCK | O_NOCTTY | O_CLOEXEC);
    if (output_fd < 0) return -1;
    interrupted = 0;
    struct sigaction action = {0};
    action.sa_handler = stop;
    sigemptyset(&action.sa_mask);
    for (size_t i = 0; i < sizeof(signals) / sizeof(signals[0]); ++i) {
        if (sigaction(signals[i], &action, &previous[i]) < 0) {
            int error = errno;
            gm_terminal_close();
            errno = error;
            return -1;
        }
        ++registered;
    }
    struct termios raw = saved;
    cfmakeraw(&raw);
    if (tcsetattr(STDIN_FILENO, TCSANOW, &raw) < 0) {
        int error = errno;
        gm_terminal_close();
        errno = error;
        return -1;
    }
    active = 1;
    return 0;
}

int gm_terminal_stopped(void) { return interrupted; }
void gm_terminal_stop(void) { interrupted = 1; }

ssize_t gm_terminal_write(const unsigned char *bytes, size_t len) {
    return write(output_fd, bytes, len);
}

int gm_terminal_wait_output(int timeout_ms, int watch_input) {
    struct pollfd fds[] = {
        {.fd = output_fd, .events = POLLOUT},
        {.fd = STDIN_FILENO, .events = POLLIN},
    };
    int result = poll(fds, watch_input ? 2 : 1, timeout_ms);
    if (result < 0) return errno == EINTR ? 0 : -1;
    if (fds[0].revents & (POLLHUP | POLLERR | POLLNVAL)) {
        errno = EIO;
        return -1;
    }
    return result;
}

int gm_terminal_size(unsigned short *cols, unsigned short *rows,
                     unsigned short *width, unsigned short *height) {
    struct winsize size = {0};
    if (ioctl(STDOUT_FILENO, TIOCGWINSZ, &size) < 0) return -1;
    *cols = size.ws_col;
    *rows = size.ws_row;
    *width = size.ws_xpixel;
    *height = size.ws_ypixel;
    return 0;
}

int gm_terminal_read(unsigned char *bytes, size_t len, int timeout_ms) {
    struct pollfd fd = {.fd = STDIN_FILENO, .events = POLLIN};
    int result = poll(&fd, 1, timeout_ms);
    if (result < 0) return errno == EINTR ? 0 : -1;
    if (!result) return 0;
    if (fd.revents & (POLLHUP | POLLERR | POLLNVAL)) {
        interrupted = 1;
        return 0;
    }
    ssize_t count = read(STDIN_FILENO, bytes, len);
    if (count == 0) interrupted = 1;
    if (count < 0 && errno == EINTR) return 0;
    return (int)count;
}
