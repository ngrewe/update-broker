#include <apt-pkg/init.h>
#include <apt-pkg/configuration.h>
#include <apt-pkg/fileutl.h>
#include <apt-pkg/error.h>
#include <cstdlib>
#include <cstdio>
#include <cstring>
#include <string>
#include <mutex>


extern "C" {

  std::mutex _apt_c_sys_lock;
  void apt_c_init_system() {
    if (!_system) {
      _apt_c_sys_lock.lock();
      try {
        if (_system) {
          _apt_c_sys_lock.unlock();
          return;
        }
        pkgInitConfig(*_config);
        pkgInitSystem(*_config, _system);
      } catch (void* any) {
         _apt_c_sys_lock.unlock();
        throw any;
      }
      _apt_c_sys_lock.unlock();
    }
  }
  char* apt_c_owned_string(std::string str) {
    char* storage = (char*)malloc((str.length() + 1) * sizeof(char));
    if (!storage) {
      return NULL;
    }
    return std::strcpy(storage, str.c_str());
  }

  void apt_c_free_string(char* str) {
    if (str) {
      free(str);
    }
  }

  char* apt_c_config_get_owned_str(const char* key, const char* def) {
    std::string value = _config->Find(key, def);
    return apt_c_owned_string(value);
  }

  char* apt_c_config_get_owned_file_path(const char* key, const char* def) {
    std::string value = _config->FindFile(key, def);
    return apt_c_owned_string(value);
  }

  bool apt_c_config_get_bool(const char* key, bool def) {
    return _config->FindB(key, def);
  }

  int apt_c_config_get_int(const char* key, int def) {
    return _config->FindI(key, def);
  }

  int apt_c_get_lock(const char* file) {
    return GetLock(std::string(file));
  }

  char* apt_c_pop_last_error_owned() {
    std::string result;
    if (!_error->PopMessage(result)) {
      return NULL;
    }
    return apt_c_owned_string(result);
  }

}
