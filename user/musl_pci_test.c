#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
#include <unistd.h>
#include <dirent.h>
#include <sys/stat.h>
#include <errno.h>

void test_stat_root() {
    struct stat st;
    if (stat("/", &st) == 0) {
        if (S_ISDIR(st.st_mode)) {
            printf("PASS: stat(/) is a directory\n");
        } else {
            printf("FAIL: stat(/) is not a directory (mode %o)\n", st.st_mode);
        }
    } else {
        printf("FAIL: stat(/) failed (errno %d)\n", errno);
    }
}

void test_proc_self_exe() {
    char buf[128];
    ssize_t len = readlink("/proc/self/exe", buf, sizeof(buf) - 1);
    if (len > 0) {
        buf[len] = '\0';
        printf("PASS: readlink(/proc/self/exe) = %s\n", buf);
        
        int fd = open("/proc/self/exe", O_RDONLY);
        if (fd >= 0) {
            unsigned char magic[4];
            if (read(fd, magic, 4) == 4) {
                if (magic[0] == 0x7f && magic[1] == 'E' && magic[2] == 'L' && magic[3] == 'F') {
                    printf("PASS: /proc/self/exe has ELF magic\n");
                } else {
                    printf("FAIL: /proc/self/exe magic mismatch: %02x%02x%02x%02x\n", magic[0], magic[1], magic[2], magic[3]);
                }
            } else {
                printf("FAIL: read(/proc/self/exe) failed\n");
            }
            close(fd);
        } else {
            printf("FAIL: open(/proc/self/exe) failed (errno %d)\n", errno);
        }
    } else {
        printf("FAIL: readlink(/proc/self/exe) failed (errno %d)\n", errno);
    }
}

void test_pci_sysfs() {
    DIR *dir = opendir("/sys/bus/pci/devices");
    if (!dir) {
        printf("FAIL: opendir(/sys/bus/pci/devices) failed (errno %d)\n", errno);
        return;
    }

    printf("PCI Devices:\n");
    struct dirent *de;
    int count = 0;
    while ((de = readdir(dir)) != NULL) {
        if (strcmp(de->d_name, ".") == 0 || strcmp(de->d_name, "..") == 0) continue;

        char path[256];
        snprintf(path, sizeof(path), "/sys/bus/pci/devices/%s/vendor", de->d_name);
        
        int fd = open(path, O_RDONLY);
        if (fd >= 0) {
            char vendor[16];
            ssize_t n = read(fd, vendor, sizeof(vendor) - 1);
            if (n > 0) {
                vendor[n] = '\0';
                // Trim newline
                char *nl = strchr(vendor, '\n');
                if (nl) *nl = '\0';
                
                printf("  Device %s: Vendor %s\n", de->d_name, vendor);
                count++;
            }
            close(fd);
        }
    }
    closedir(dir);
    
    if (count > 0) {
        printf("PASS: Found %d PCI devices in sysfs\n", count);
    } else {
        printf("FAIL: No PCI devices found or files unreadable\n");
    }
}

int main() {
    printf("--- musl_pci_test start ---\n");
    
    test_stat_root();
    test_proc_self_exe();
    test_pci_sysfs();
    
    printf("--- musl_pci_test finished ---\n");
    return 0;
}
