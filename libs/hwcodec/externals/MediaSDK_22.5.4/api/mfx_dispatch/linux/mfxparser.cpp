// Copyright (c) 2017-2019 Intel Corporation
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <list>

#include "mfxloader.h"

namespace MFX {

static bool parseGUID(const char* src, mfxPluginUID* uid)
{
    mfxPluginUID plugin_uid{};
    mfxU8* p = plugin_uid.Data;

    int res = sscanf(src,
        "%02hhx%02hhx%02hhx%02hhx%02hhx%02hhx%02hhx%02hhx%02hhx%02hhx%02hhx%02hhx%02hhx%02hhx%02hhx%02hhx",
        p, p + 1, p + 2, p + 3, p + 4, p + 5, p + 6, p + 7, 
        p + 8, p + 9, p + 10, p + 11, p + 12, p + 13, p + 14, p + 15);

    if (res != sizeof(uid->Data)) {
        return false;
    }

    *uid = plugin_uid;
    return true;
}

static std::string printUID(const mfxPluginUID& uid)
{
  std::stringstream ss;
  ss << std::hex;
  for (auto c: uid.Data) ss << static_cast<unsigned>(c);
  return ss.str();
}

static std::string printCodecId(mfxU32 id)
{
  uint8_t* data = reinterpret_cast<uint8_t*>(&id);
  std::stringstream ss;
  for (size_t i=0; i < sizeof(id); ++i) ss << data[i];
  return ss.str();
}

void PluginInfo::Load(const char* name, const char* value)
{
#ifdef LINUX64
    #define FIELD_FileName "FileName64"
#else
    #define FIELD_FileName "FileName32"
#endif

    if (!strcmp(name, "Type")) {
        Type = atoi(value);
        m_parsed |= PARSED_TYPE;
    } else if (!strcmp(name, "CodecID")) {
        const int fourccLen = 4;
        if (strlen(value) == 0 || strlen(value) > fourccLen)
            return;

        CodecId = MFX_MAKEFOURCC(' ',' ',' ',' ');
        char* id = reinterpret_cast<char*>(&CodecId);
        for (size_t i = 0; i < strlen(value); ++i)
            id[i] = value[i];

        m_parsed |= PARSED_CODEC_ID;
    } else if (!strcmp(name, "GUID")) {
        if (!parseGUID(value, &PluginUID))
            return;

        m_parsed |= PARSED_UID;
    } else if (!strcmp(name, "Path") || !strcmp(name, FIELD_FileName)) {
        // strip quotes
        std::string str_value(value);

        if (!str_value.empty() && str_value.front() == '"' && str_value.back() == '"')
        {
            str_value.pop_back();

            if (!str_value.empty())
                str_value.erase(0, 1);
        }

        if (strlen(m_path) + strlen("/") + str_value.size() >= PATH_MAX)
            return;
        strncpy(m_path + strlen(m_path), str_value.c_str(), str_value.size() + 1);
        m_parsed |= PARSED_PATH;
    } else if (0 == strcmp(name, "Default")) {
        m_default = (0 != atoi(value));
        m_parsed |= PARSED_DEFAULT;
    } else if (0 == strcmp(name, "PluginVersion")) {
        PluginVersion = atoi(value);
        m_parsed |= PARSED_VERSION;
    } else if (0 == strcmp(name, "APIVersion")) {
        APIVersion.Version = atoi(value);
        m_parsed |= PARSED_API_VERSION;
    }
}

void PluginInfo::Print()
{
  printf("[%s]\n", printUID(PluginUID).c_str());
  printf("  GUID=%s\n", printUID(PluginUID).c_str());
  printf("  PluginVersion=%d\n", PluginVersion);
  printf("  APIVersion=%d\n", APIVersion.Version);
  printf("  Path=%s\n", m_path);
  printf("  Type=%d\n", Type);
  printf("  CodecID=%s\n", printCodecId(CodecId).c_str());
  printf("  Default=%d\n", m_default);
}

const std::string space_search_pattern(" \f\n\r\t\v");
// strip tailing spaces
void strip(std::string & str)
{
    static_assert(std::string::npos + 1 == 0, "");
    str.erase(str.find_last_not_of(space_search_pattern) + 1);
}

// skip initial spaces
void skip(std::string & str)
{
    str.erase(0, str.find_first_not_of(space_search_pattern));
}

void parse(const char* file_name, std::list<PluginInfo>& plugins)
{
#if (__GLIBC__ > 2) || (__GLIBC__ == 2 && __GLIBC_MINOR__ >= 7)
  const char* mode = "re";
#else
  const char* mode = "r";
#endif

  FILE* file = fopen(file_name, mode);
  if (!file)
    return;

  char c_line[PATH_MAX];
  PluginInfo plg;
  std::string line;

  while(fgets(c_line, PATH_MAX, file))
  {
    line = c_line;

    strip(line);
    skip(line);

    if (line.find_first_not_of(";#") != 0)
    {
      // skip comments
      continue;
    }
    else if (line[0] == '[')
    {
      if (plg.isValid()) {
        plugins.push_back(std::move(plg));
        plg = PluginInfo{};
      }
    }
    else
    {
      std::string name = line, value = line;

      size_t pos = value.find_first_of("=:");
      if (pos != std::string::npos)
      {
          // Get left part relative to delimiter
          name.erase(pos);
          strip(name);

          // Get right part relative to delimiter
          value.erase(0, pos + 1);
          skip(value);

          static_assert(std::string::npos + 1 == 0, "");
          value.erase(value.find_last_not_of(";#") + 1);
      }
      if (!name.empty() && !value.empty()) {
        plg.Load(name.c_str(), value.c_str());
      }
    }
  }

  if (plg.isValid()) {
      plugins.push_back(std::move(plg));
  }

  fclose(file);

  //print(plugins); // for debug
}

} // namespace MFX
