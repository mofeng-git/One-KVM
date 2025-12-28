// 
// Notice Regarding Standards.  AMD does not provide a license or sublicense to
// any Intellectual Property Rights relating to any standards, including but not
// limited to any audio and/or video codec technologies such as MPEG-2, MPEG-4;
// AVC/H.264; HEVC/H.265; AAC decode/FFMPEG; AAC encode/FFMPEG; VC-1; and MP3
// (collectively, the "Media Technologies"). For clarity, you will pay any
// royalties due for such third party technologies, which may include the Media
// Technologies that are owed as a result of AMD providing the Software to you.
// 
// MIT license 
// 
//
// Copyright (c) 2018 Advanced Micro Devices, Inc. All rights reserved.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.  IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.
//


#include "JsonImpl.h"
#include <stdio.h>
#include <cstdlib>
#include <sstream>
#include <inttypes.h>

#pragma warning(disable: 4996)

static amf::JSONParser::OutputFormatDesc defaultFormat = {};

///////////////////////////// Element ////////////////////////////////////////
amf::JSONParserImpl::ElementHelper::ElementHelper()
{
}

size_t amf::JSONParserImpl::ElementHelper::FindClosure(const std::string& str, char opener, char closer, size_t start)
{
    size_t endPos = start;
    
    if (opener != closer)
    {
        int openerCnt = 0;
        int closerCnt = 0;

        bool inQuote = false;
        bool backslashEscape = false;

        for (size_t i = start; i < str.length(); i++)
        {
            char sym = str.at(i);

            // skip openers/closers that are inside quotes
            if (sym == '\\' && backslashEscape == false) {
                backslashEscape = true;
                continue;
            }
            if (sym == '"' && backslashEscape == false)
            {
                inQuote = !inQuote;
            }

            backslashEscape = false;
            if (inQuote == true)
            {
                continue;
            }

            if (sym == opener)
            {
                ++openerCnt;
            }
            else if (sym == closer)
            {
                ++closerCnt;
                if (openerCnt == closerCnt)
                {
                    endPos = i;
                    break;
                }
            }
        }
    }
    else
    {
        for (size_t i = start+1; i < str.length(); i++)
        {
            if (str.at(i) == closer)
            {
                endPos = i;
                break;
            }
        }
    }
    return endPos;
}

amf::JSONParser::Error amf::JSONParserImpl::ElementHelper::CreateElement(const std::string& str, size_t start, size_t& valueStart, size_t& valueEnd, JSONParser::Element** val)
{
    *val = nullptr;
    static const char* specialCharsStart = "\t\n\r ";
    static const char* specialCharsEnd = "\t\n\r,:}] ";
    valueStart = str.find_first_not_of(specialCharsStart, start);
    bool bParse = true;
    if (valueStart == str.npos)
    {
        return Error(start, JSONParser::MISSING_VALUE);
    }
    else
    {
        switch (str.at(valueStart))
        {
        case '{':
            valueEnd = FindClosure(str, '{', '}', valueStart);
            if (valueEnd == str.npos)
            {
                return Error(start, JSONParser::MISSING_BRACE);
            }
            else
            {
                ++valueEnd;
            }
            *val = new NodeImpl();
            break;
        case '[':
            valueEnd = FindClosure(str, '[', ']', valueStart);
            if (valueEnd == str.npos)
            {
                return Error(start, JSONParser::MISSING_BRACKET);

            }
            else
            {
                ++valueEnd;
            }
            *val = new ArrayImpl();
        
            break;
            
        case '\"':
        {
            valueEnd = FindClosure(str, '\"', '\"', valueStart);
            if (valueEnd == str.npos)
            {
                return Error(start, JSONParser::MISSING_QUOTE);
            }
            else
            {
                ++valueEnd;
            }
            ValueImpl *valImpl = new ValueImpl();
            valImpl->SetValue(str.substr(valueStart +1, valueEnd - valueStart - 2) );
            *val = valImpl;
            bParse = false;
            break;
        }
        
        default:
            {
           
                if (valueStart == str.npos)
                {
                    return Error(start, JSONParser::MISSING_VALUE);
                }
                else
                {
                    valueEnd = str.find_first_of(specialCharsEnd, valueStart);
                    if (valueEnd == str.npos)
                    {
                        return Error(valueStart, JSONParser::MISSING_DELIMITER);
                    }
                }
                *val = new ValueImpl();
                break;
        }
        }
    
        if (*val != nullptr && bParse == true)
        {
            (*val)->Parse(str, valueStart, valueEnd);
        }
    }
    return Error(valueStart, JSONParser::OK);
}

void amf::JSONParserImpl::ElementHelper::InsertTabs(std::string& target, int count, const OutputFormatDesc& format) const
{
    if (format.bHumanReadable)
    {
        int whitespacesToInsert = count * format.nOffsetSize;
        for (int i = 0; i < whitespacesToInsert; i++)
        {
            target += format.cOffsetWith;
        }
    }
}

///////////////////////////// Value ////////////////////////////////////////
static const char* const NULL_STR = "null";
static const char* const TRUE_STR = "true";
static const char* const FALSE_STR = "false";
amf::JSONParserImpl::ValueImpl::ValueImpl() :
    ElementHelper(),
    m_eType(VT_Unknown)
{
}

bool amf::JSONParserImpl::ValueImpl::IsNull() const
{
    return m_eType == VT_Null;
}

const std::string& amf::JSONParserImpl::ValueImpl::GetValue() const
{
    return m_Value;
}

void amf::JSONParserImpl::ValueImpl::SetValue(const std::string& val)
{
    m_Value = val;
    m_eType = VT_String;
}

void amf::JSONParserImpl::ValueImpl::SetValueAsInt32(int32_t val)
{
    std::stringstream str;
    str << val;
    m_Value = str.str();
    m_eType = VT_Numeric;
}

void amf::JSONParserImpl::ValueImpl::SetValueAsUInt32(uint32_t val)
{
    std::stringstream str;
    str << val;
    m_Value = str.str();
    m_eType = VT_Numeric;
}

void amf::JSONParserImpl::ValueImpl::SetValueAsInt64(int64_t val)
{
    std::stringstream str;
    str << val;
    m_Value = str.str();
    m_eType = VT_Numeric;
}

void amf::JSONParserImpl::ValueImpl::SetValueAsUInt64(uint64_t val)
{
    std::stringstream str;
    str << val;
    m_Value = str.str();
    m_eType = VT_Numeric;
}

void amf::JSONParserImpl::ValueImpl::SetValueAsDouble(double val)
{
    char buf[100];
    sprintf(buf, "%.16lf", val);
    m_Value = buf;
    if (m_Value.compare("-nan(ind)") == 0)
    {
        SetToNull();
    }
    else
    {
        m_eType = VT_Numeric;
    }

    //trim trailing zeroes
    if (m_Value.find_first_of(".") != std::string::npos)
    {
        std::size_t found = m_Value.find_last_not_of("0");
        if (found != std::string::npos)
        {
            if (m_Value[found] == '.')
                m_Value.erase(found);
            else
                m_Value.erase(found + 1);
        }
        else // case value == 0
        {
            m_Value.erase(1);
        }
    }
}

void amf::JSONParserImpl::ValueImpl::SetValueAsFloat(float val)
{
    SetValueAsDouble(static_cast<double>(val));
}

void amf::JSONParserImpl::ValueImpl::SetValueAsBool(bool val)
{
    m_Value = val ? TRUE_STR : FALSE_STR;
    m_eType = VT_Bool;
}

void amf::JSONParserImpl::ValueImpl::SetValueAsTime(time_t date, bool utc)
{
    int64_t val = (utc == true) ? date : std::mktime(std::localtime(&date));
    std::stringstream str;
    str << val;
    m_Value = str.str();
    m_eType = VT_Numeric;
}

void amf::JSONParserImpl::ValueImpl::SetToNull()
{
    m_Value = NULL_STR;
    m_eType = VT_Null;
}


int32_t amf::JSONParserImpl::ValueImpl::GetValueAsInt32() const
{
    int val = 0;
    if(!m_Value.empty())
    {
        val = strtol(m_Value.c_str(), nullptr, 10);
    }
    return (int32_t)val;
}
uint32_t amf::JSONParserImpl::ValueImpl::GetValueAsUInt32() const
{
    unsigned int val = 0;
    if(!m_Value.empty())
    {
        val = strtoul(m_Value.c_str(), nullptr, 10);
    }
    return (uint32_t)val;
}
int64_t amf::JSONParserImpl::ValueImpl::GetValueAsInt64() const
{
    long long val = 0;
    if(!m_Value.empty())
    {
        val = strtoll(m_Value.c_str(), nullptr, 10);
    }
    return val;
}

uint64_t amf::JSONParserImpl::ValueImpl::GetValueAsUInt64() const
{
    uint64_t val = 0;
    if(!m_Value.empty())
    {
        val = strtoull(m_Value.c_str(), nullptr, 10);
    }
    return val;
}

double amf::JSONParserImpl::ValueImpl::GetValueAsDouble() const
{
    double val = 0;
    if (!m_Value.empty())
    {
        val = strtod(m_Value.c_str(), nullptr);
    }
    return val;
}
float amf::JSONParserImpl::ValueImpl::GetValueAsFloat() const
{
    float val = 0;
    if(!m_Value.empty())
    {
        val = static_cast<float>(strtod(m_Value.c_str(), nullptr));
    }
    return val;
}
bool amf::JSONParserImpl::ValueImpl::GetValueAsBool() const
{
    bool retVal = false;
    if (!m_Value.empty())
    {
        if (m_eType == VT_Bool)
        {
            retVal = (m_Value.compare(TRUE_STR) == 0);
        }
        else
        {
            double val = 0;
            val = strtod(m_Value.c_str(), nullptr);
            retVal = val != 0;
        }
    }
    return retVal;
}

time_t amf::JSONParserImpl::ValueImpl::GetValueAsTime() const
{
    if (!m_Value.empty() && m_eType == VT_String)
    {
        return strtoll(m_Value.c_str(), nullptr, 10);
    }

    return 0;
}


amf::JSONParser::Error amf::JSONParserImpl::ValueImpl::Parse(const std::string& str, size_t start, size_t end)
{
    static const char* specialCharacters = "\"\'\n\r,[{}]"; // whitespaces excluded
    if(start == end)
    {
        m_Value = "";
        m_eType = VT_String;
    }
    else
    {
        size_t startPos = str.find_first_not_of(specialCharacters, start);
        if (startPos == str.npos)
        {
            return Error(start, JSONParser::MISSING_VALUE);
        }
        startPos = str.find_first_not_of(" \t", startPos); // trim start whitespaces
        if (startPos == str.npos)
        {
            return Error(start, JSONParser::MISSING_VALUE);
        }
        size_t endPos = str.find_first_of(specialCharacters, startPos+1); // exclude spaces from search
        if (endPos == str.npos)
        {
            return Error(start, JSONParser::MISSING_VALUE);
        }
        endPos = str.find_last_not_of(" \t", endPos); // trim end whitespaces
        if (endPos == str.npos)
        {
            return Error(start, JSONParser::MISSING_VALUE);
        }
        m_Value.assign(str, startPos, endPos - startPos);
        // Determine Special Types
        if(m_Value.compare(NULL_STR) == 0)
        {
            m_eType = VT_Null;
        }
        else if(m_Value.compare(TRUE_STR) == 0 || m_Value.compare(FALSE_STR) == 0)
        {
            m_eType = VT_Bool;
        }
        else
        {
            m_eType = VT_Numeric;
        }

    }
    return Error(start, JSONParser::OK);
}

std::string amf::JSONParserImpl::ValueImpl::Stringify() const
{
    return StringifyFormatted(defaultFormat, 0);
}

std::string amf::JSONParserImpl::ValueImpl::StringifyFormatted(const OutputFormatDesc&, int /*indent*/) const
{
    std::string jsonValue;
    if ((m_eType == VT_String || m_Value.length() == 0) && IsNull() == false)
    {
        jsonValue += '\"';
    }
    jsonValue += m_Value;
    if ((m_eType == VT_String || m_Value.length() == 0) && IsNull() == false)
    {
        jsonValue += '\"';
    }
    return jsonValue;
}

///////////////////////////// Node ////////////////////////////////////////
amf::JSONParserImpl::NodeImpl::NodeImpl() :
    ElementHelper()
{
}

size_t amf::JSONParserImpl::NodeImpl::GetElementCount() const
{
    return m_Elements.size();
}

amf::JSONParser::Error amf::JSONParserImpl::NodeImpl::Parse(const std::string& str, size_t start, size_t end)
{
    size_t curHead = start;
    bool continueParsing = true;
    
    do
    {
        size_t nameStart = str.find_first_of('\"', curHead);
        size_t nextClosingBrace = str.find_first_of('}', curHead);

        // Additional check for empty elements:
        // If empty element isn't last in the file
        // '"' character check might fail to parse correctly
        // resulting in adding next elements as children
        if (nextClosingBrace < nameStart)
        {
            return Error(start, JSONParser::OK);
        }

        if (nameStart == str.npos)
        {
            if(start + 1 == end)
            {
                return Error(start, JSONParser::OK);
            }
            return Error(start, JSONParser::MISSING_QUOTE);
        }

        size_t nameEnd = str.find_first_of('\"', nameStart + 1);
        if (nameEnd == str.npos)
        {
            return Error(nameStart, JSONParser::MISSING_QUOTE);
        }

        std::string name;
        name.assign(str, nameStart + 1, nameEnd - nameStart - 1);
        size_t delimPos = str.find_first_of(':', nameEnd + 1);
        if (delimPos == str.npos)
        {
            return Error(nameStart, JSONParser::MISSING_DELIMITER);
        }

        size_t valueStart = delimPos+1; //str.find_first_of("\"[{", delimPos + 1);
        if (valueStart == str.npos)
        {
            return Error(nameStart, JSONParser::MISSING_VALUE);
        }
        size_t valueEnd = 0;
        Element* val;
        Error createErr = CreateElement(str, valueStart, valueStart, valueEnd, &val);
        if (createErr.GetResult() != OK)
        {
            return createErr;
        }
        
        size_t commaPos = str.find_first_not_of(" \t\n\r", valueEnd);
        if (commaPos == str.npos)
        {
            return Error(nameStart, JSONParser::UNEXPECTED_END);
        }
        else if (str.at(commaPos) != ',')
        {
            continueParsing = false;
        }
        else
        {
            curHead = commaPos+1;
        }
        JSONParser::Result res = AddElement(name, val);
        if (res != JSONParser::OK)
        {
            return Error(nameStart, res);
        }
    } 
    while (continueParsing == true);
    return Error(start, JSONParser::OK);
}

std::string amf::JSONParserImpl::NodeImpl::Stringify() const
{
    return StringifyFormatted(defaultFormat, 0);
}

std::string amf::JSONParserImpl::NodeImpl::StringifyFormatted(const OutputFormatDesc& format, int indent) const
{
    bool first = true;
    std::string jsonValue;

    InsertTabs(jsonValue, indent, format);
    jsonValue += '{';
    for (ElementMap::const_iterator it = m_Elements.begin(); it != m_Elements.end(); ++it)
    {
        if (first == false)
        {
            jsonValue += ',';
        }
        else
        {
            first = false;
        }
        if (format.bHumanReadable == true)
        {
            jsonValue += '\n';
        }
        InsertTabs(jsonValue, indent + 1, format);
        
        jsonValue += '\"';
        jsonValue += it->first;
        jsonValue += format.bHumanReadable == true ? "\" : " : "\":";
        if (format.bHumanReadable == true)
        {
            amf::JSONParserImpl::Value::Ptr value(it->second);
            if (value == nullptr && format.bNewLineBeforeBrace == true)
            {
                jsonValue += '\n';
            }
        }
        jsonValue += it->second == nullptr ? "null" : it->second->StringifyFormatted(format, indent + 1);
    }
    if (format.bHumanReadable == true && format.bNewLineBeforeBrace == true)
    {
        jsonValue += '\n';
    }
    InsertTabs(jsonValue, indent, format);
    jsonValue += '}';
    return jsonValue;
}

amf::JSONParser::Element* amf::JSONParserImpl::NodeImpl::GetElementByName(const std::string& name) const
{
    Element* found(nullptr);
    ElementMap::const_iterator it = m_Elements.find(name);
    if (it != m_Elements.end())
    {
        found = it->second;
    }
    return found;
}

amf::JSONParser::Result amf::JSONParserImpl::NodeImpl::AddElement(const std::string& name, amf::JSONParser::Element* element)
{
    JSONParser::Result result = OK;
    if (m_Elements.find(name) == m_Elements.end())
    {
        m_Elements.insert(std::pair<std::string, Element::Ptr>(name, Element::Ptr(element)));
    }
    else
    {
        result = JSONParser::DUPLICATE_NAME;
    }
    return result;
}

amf::JSONParser::Element* amf::JSONParserImpl::NodeImpl::GetElementAt(size_t idx, std::string& name) const
{
    if (m_Elements.size() <= idx)
    {
        return nullptr;
    }
    ElementMap::const_iterator it = m_Elements.begin();
    for (size_t i = 0; i < idx; i++)
    {
        it++;
    }
    name = it->first;
    return it->second;
}

///////////////////////////// Array ////////////////////////////////////////
amf::JSONParserImpl::ArrayImpl::ArrayImpl() :
    ElementHelper()
{
}

amf::JSONParser::Error amf::JSONParserImpl::ArrayImpl::Parse(const std::string& str, size_t start, size_t end)
{
    bool continueParsing = true;
    size_t valueStart;
    size_t valueEnd = start+1;
    do
    {
        valueStart = str.find_first_not_of("\t\r\n", valueEnd);
        if (valueStart == str.npos)
        {
            return Error(valueEnd, JSONParser::MISSING_VALUE);
        }
        if(valueStart + 1 == end)
        {
            break;
        }
        Element* val = nullptr;
        Error createErr = CreateElement(str, valueStart, valueStart, valueEnd, &val);
        if (createErr.GetResult() != OK)
        {
            return createErr;
        }
        AddElement(Element::Ptr(val));
        size_t commaPos = str.find_first_not_of(" \t\n\r", valueEnd);
        if (commaPos == str.npos)
        {
            return Error(valueStart, JSONParser::UNEXPECTED_END);
        }
        else if (str.at(commaPos) != ',')
        {
            continueParsing = false;
        }
        else
        {
            valueEnd = commaPos+1;
        }
    } while (continueParsing == true);
    return Error(valueStart, JSONParser::OK);
}

void amf::JSONParserImpl::ArrayImpl::AddElement(Element* element)
{
    m_Elements.push_back(Element::Ptr(element));
}

std::string amf::JSONParserImpl::ArrayImpl::Stringify() const
{
    return StringifyFormatted(defaultFormat, 0);
}

std::string amf::JSONParserImpl::ArrayImpl::StringifyFormatted(const OutputFormatDesc& format, int indent) const
{
    bool first = true;
    std::string jsonValue;
    InsertTabs(jsonValue, indent, format);
    
    jsonValue += '[';
    bool newLineBeforeClosingBrace = false;
    for (ElementVector::const_iterator it = m_Elements.begin(); it != m_Elements.end(); ++it)
    {
        if (first == false)
        {
            jsonValue += ',';
        }
        else
        {
            first = false;
        }
        if (format.bHumanReadable == true)
        {
            amf::JSONParserImpl::Node::Ptr node(*it);
            if (node != nullptr)
            {
                jsonValue += '\n';
                newLineBeforeClosingBrace = true;
            }
        }
        jsonValue += *it == nullptr ? "null" : (*it)->StringifyFormatted(format, indent + 1);
    }
    if (format.bHumanReadable == true && newLineBeforeClosingBrace == true)
    {
        jsonValue += '\n';
    }
    InsertTabs(jsonValue, indent, format);
    
    jsonValue += ']';

    return jsonValue;

}

size_t amf::JSONParserImpl::ArrayImpl::GetElementCount() const
{
    return m_Elements.size();
}

amf::JSONParser::Element* amf::JSONParserImpl::ArrayImpl::GetElementAt(size_t idx) const
{
    return m_Elements[idx];
}

///////////////////////////// JSONParser ////////////////////////////////////////
amf::JSONParserImpl::JSONParserImpl() :
    m_LastErrorOfs(0)
{
}

amf::JSONParser::Result amf::JSONParserImpl::Parse(const std::string& str, amf::JSONParser::Node** root)
{
    amf::JSONParser::Result result = OK;
    if (root == nullptr)
    {
        result = INVALID_ARG;
    }
    else
    {
        amf::JSONParser::Node* rootNode(nullptr);
        size_t start = str.find_first_of('{'); 
        size_t end = str.find_last_of('}', str.length());
        if (start != str.npos && end != str.npos)
        {
            rootNode = new NodeImpl();
            Error parseErr = rootNode->Parse(str, start, end);
            if (parseErr.GetResult() != OK)
            {
                result = parseErr.GetResult();
                m_LastErrorOfs = parseErr.GetOffset();
            }
            else
            {
                *root = rootNode;
                (*root)->Acquire();
            }
        }
        else
        {
            result = MISSING_BRACE;
        }
    }
    return result;
}
    
std::string amf::JSONParserImpl::Stringify(const JSONParser::Node* root) const
{
    return StringifyFormatted(root, defaultFormat, 0);
}

std::string amf::JSONParserImpl::StringifyFormatted(const Node* root, const OutputFormatDesc& format, int indent) const
{
    std::string jsonStr;
    if (root != nullptr)
    {
        jsonStr = root->StringifyFormatted(format, indent);
    }
    return jsonStr;
}

size_t amf::JSONParserImpl::GetLastErrorOffset() const
{
    return m_LastErrorOfs;
}

amf::JSONParserImpl::Result amf::JSONParserImpl::CreateNode(Node** node) const 
{
    Result result = INVALID_ARG;
    if (node != nullptr)
    {
        *node = new NodeImpl();
        (*node)->Acquire();
        result = OK;
    }
    return result;
}

amf::JSONParserImpl::Result amf::JSONParserImpl::CreateValue(Value** value) const 
{
    Result result = INVALID_ARG;
    if (value != nullptr)
    {
        *value = new ValueImpl();
        (*value)->Acquire();
        result = OK;
    }
    return result;
}

amf::JSONParserImpl::Result amf::JSONParserImpl::CreateArray(Array** array) const 
{
    Result result = INVALID_ARG;
    if (array != nullptr)
    {
        *array = new ArrayImpl();
        (*array)->Acquire();
        result = OK;
    }
    return result;
}

extern "C"
{
    AMF_RESULT AMF_CDECL_CALL CreateJSONParser(amf::JSONParser** parser)
    {
        AMF_RESULT result;
        if (parser != nullptr)
        {
            *parser = new amf::JSONParserImpl();
            (*parser)->Acquire();
            result = AMF_OK;
        }
        else
        {
            result = AMF_INVALID_ARG;
        }
        return result;
    }

#define JSON_SET_VALUE(CREATOR, parser, root, name, val) do { \
        amf::JSONParser::Value::Ptr node; \
        CREATOR(parser, &node, val); \
        root->AddElement(name, node); \
    } while(0)

#define JSON_SET_ARRAY(CREATOR, parser, root, name, val, size) do { \
        if (size == 0) { return; } \
        amf::JSONParser::Array::Ptr node; \
        CREATOR(parser, &node, val, size); \
        root->AddElement(name, node); \
    } while(0)

#define JSON_SET_OBJECT(CREATOR, parser, root, name, val) do { \
        amf::JSONParser::Array::Ptr node; \
        CREATOR(parser, &node, val); \
        root->AddElement(name, node); \
    } while(0)

    void amf::SetDoubleValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, double val)
    {
        JSON_SET_VALUE(CreateDoubleValue, parser, root, name, val);
    }

    void amf::CreateDoubleValue(amf::JSONParser* parser, amf::JSONParser::Value** node, double val)
    {
        parser->CreateValue(node);
        (*node)->SetValueAsDouble(val);
    }

    void amf::SetFloatValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, float val)
    {
        JSON_SET_VALUE(CreateFloatValue, parser, root, name, val);
    }

    void amf::CreateFloatValue(amf::JSONParser* parser, amf::JSONParser::Value** node, float val)
    {
        parser->CreateValue(node);
        (*node)->SetValueAsFloat(val);
    }

    void amf::SetInt64Array(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const int64_t* val, size_t size)
    {
        if (size == 0)
        {
            return;
        }
        amf::JSONParser::Array::Ptr array;
        parser->CreateArray(&array);
        for (size_t i = 0; i < size; i++)
        {
            amf::JSONParser::Value::Ptr node;
            parser->CreateValue(&node);
            node->SetValueAsInt64(val[i]);
            array->AddElement(node);
        }
        root->AddElement(name, array);
    }

    void amf::SetInt32Array(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const int32_t* val, size_t size)
    {
        JSON_SET_ARRAY(CreateInt32Array, parser, root, name, val, size);
    }

    void amf::CreateInt32Array(amf::JSONParser* parser, amf::JSONParser::Array** array, const int32_t* val, size_t size)
    {
        parser->CreateArray(array);
        for (size_t i = 0; i < size; i++)
        {
            amf::JSONParser::Value::Ptr node;
            parser->CreateValue(&node);
            node->SetValueAsInt32(val[i]);
            (*array)->AddElement(node);
        }
    }

    void amf::SetUInt32Array(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const uint32_t* val, size_t size)
    {
        JSON_SET_ARRAY(CreateUInt32Array, parser, root, name, val, size);
    }

    void amf::CreateUInt32Array(amf::JSONParser* parser, amf::JSONParser::Array** array, const uint32_t* val, size_t size)
    {
        parser->CreateArray(array);
        for (size_t i = 0; i < size; i++)
        {
            amf::JSONParser::Value::Ptr node;
            parser->CreateValue(&node);
            node->SetValueAsUInt32(val[i]);
            (*array)->AddElement(node);
        }
    }

    void amf::SetFloatArray(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const float* val, size_t size)
    {
        JSON_SET_ARRAY(CreateFloatArray, parser, root, name, val, size);
    }

    void amf::CreateFloatArray(amf::JSONParser* parser, amf::JSONParser::Array** array, const float* val, size_t size)
    {
        parser->CreateArray(array);
        for (size_t i = 0; i < size; i++)
        {
            amf::JSONParser::Value::Ptr node;
            parser->CreateValue(&node);
            node->SetValueAsFloat(val[i]);
            (*array)->AddElement(node);
        }
    }

    void amf::SetDoubleArray(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const double* val, size_t size)
    {
        if (size == 0)
        {
            return;
        }
        amf::JSONParser::Array::Ptr array;
        parser->CreateArray(&array);
        for (size_t i = 0; i < size; i++)
        {
            amf::JSONParser::Value::Ptr node;
            parser->CreateValue(&node);
            node->SetValueAsDouble(val[i]);
            array->AddElement(node);
        }
        root->AddElement(name, array);
    }

    void amf::SetBoolValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, bool val)
    {
        JSON_SET_VALUE(CreateBoolValue, parser, root, name, val);
    }

    void amf::CreateBoolValue(amf::JSONParser* parser, amf::JSONParser::Value** node, bool val)
    {
        parser->CreateValue(node);
        (*node)->SetValueAsBool(val);
    }

    void amf::SetInt64Value(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, int64_t val)
    {
        JSON_SET_VALUE(CreateInt64Value, parser, root, name, val);
    }

    void amf::CreateInt64Value(amf::JSONParser* parser, amf::JSONParser::Value** node, int64_t val)
    {
        parser->CreateValue(node);
        (*node)->SetValueAsInt64(val);
    }

    void amf::SetUInt64Value(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, uint64_t val)
    {
        amf::JSONParser::Value::Ptr node;
        parser->CreateValue(&node);
        node->SetValueAsUInt64(val);
        root->AddElement(name, node);
    }

    void amf::SetInt32Value(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, int32_t val)
    {
        amf::JSONParser::Value::Ptr node;
        parser->CreateValue(&node);
        node->SetValueAsInt32(val);
        root->AddElement(name, node);
    }

    void amf::SetUInt32Value(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, uint32_t val)
    {
        amf::JSONParser::Value::Ptr node;
        parser->CreateValue(&node);
        node->SetValueAsUInt32(val);
        root->AddElement(name, node);
    }

    void amf::SetSizeValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFSize& val)
    {
        JSON_SET_OBJECT(CreateSizeValue, parser, root, name, val);
    }

    void amf::CreateSizeValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFSize& val)
    {
        int32_t arrayVal[] = { val.width, val.height };
        CreateInt32Array(parser, array, arrayVal, amf_countof(arrayVal));
    }

    void amf::SetRectValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFRect& val)
    {
        JSON_SET_OBJECT(CreateRectValue, parser, root, name, val);
    }

    void amf::CreateRectValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFRect& val)
    {
        int32_t arrayVal[] = { val.left, val.top, val.right, val.bottom };
        CreateInt32Array(parser, array, arrayVal, amf_countof(arrayVal));
    }

    void amf::SetPointValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFPoint& val)
    {
        JSON_SET_OBJECT(CreatePointValue, parser, root, name, val);
    }

    void amf::CreatePointValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFPoint& val)
    {
        int32_t arrayVal[] = { val.x, val.y };
        CreateInt32Array(parser, array, arrayVal, amf_countof(arrayVal));
    }

    void amf::SetRateValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFRate& val)
    {
        JSON_SET_OBJECT(CreateRateValue, parser, root, name, val);
    }

    void amf::CreateRateValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFRate& val)
    {
        uint32_t arrayVal[] = { val.num, val.den };
        CreateUInt32Array(parser, array, arrayVal, amf_countof(arrayVal));
    }

    void amf::SetRatioValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFRatio& val)
    {
        JSON_SET_OBJECT(CreateRatioValue, parser, root, name, val);
    }

    void amf::CreateRatioValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFRatio& val)
    {
        uint32_t arrayVal[] = { val.num, val.den };
        CreateUInt32Array(parser, array, arrayVal, amf_countof(arrayVal));
    }

    void amf::SetColorValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFColor& val)
    {
        JSON_SET_OBJECT(CreateColorValue, parser, root, name, val);
    }

    void amf::CreateColorValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFColor& val)
    {
        uint32_t arrayVal[] = { val.a, val.r, val.g, val.b };
        CreateUInt32Array(parser, array, arrayVal, amf_countof(arrayVal));
    }

    void amf::SetFloatSizeValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFFloatSize& val)
    {
        JSON_SET_OBJECT(CreateFloatSizeValue, parser, root, name, val);
    }

    void amf::CreateFloatSizeValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFFloatSize& val)
    {
        amf_float arrayVal[] = { val.width, val.height };
        CreateFloatArray(parser, array, arrayVal, amf_countof(arrayVal));
    }

    void amf::SetFloatPoint2DValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFFloatPoint2D& val)
    {
        JSON_SET_OBJECT(CreateFloatPoint2DValue, parser, root, name, val);
    }

    void amf::CreateFloatPoint2DValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFFloatPoint2D& val)
    {
        amf_float arrayVal[] = { val.x, val.y };
        CreateFloatArray(parser, array, arrayVal, amf_countof(arrayVal));
    }

    void amf::SetFloatPoint3DValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFFloatPoint3D& val)
    {
        JSON_SET_OBJECT(CreateFloatPoint3DValue, parser, root, name, val);
    }

    void amf::CreateFloatPoint3DValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFFloatPoint3D& val)
    {
        amf_float arrayVal[] = { val.x, val.y, val.z };
        CreateFloatArray(parser, array, arrayVal, amf_countof(arrayVal));
    }

    void amf::SetFloatVector4DValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFFloatVector4D& val)
    {
        JSON_SET_OBJECT(CreateFloatVector4DValue, parser, root, name, val);
    }

    void amf::CreateFloatVector4DValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFFloatVector4D& val)
    {
        amf_float arrayVal[] = { val.x, val.y, val.z, val.w };
        CreateFloatArray(parser, array, arrayVal, amf_countof(arrayVal));
    }

    void amf::SetStringValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const std::string& val)
    {
        JSON_SET_VALUE(CreateStringValue, parser, root, name, val);

    }
    void amf::CreateStringValue(amf::JSONParser* parser, amf::JSONParser::Value** node, const std::string& val)
    {
        parser->CreateValue(node);
        if (val.empty())
        {
            (*node)->SetToNull();
        }
        else
        {
            (*node)->SetValue(val);
        }
    }

    void amf::SetInterfaceValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, /*const */AMFInterface* pVal)
    {
        amf::JSONParser::Node::Ptr node;
        amf::CreateInterfaceValue(parser, &node, pVal);
        root->AddElement(name, node);
    }

    void amf::CreateInterfaceValue(amf::JSONParser* parser, amf::JSONParser::Node** node, /*const*/ AMFInterface* pVal)
    {
        parser->CreateNode(node);

        const AMFInterfaceJSONSerializablePtr p(pVal);

        if (p != nullptr)
        {
            p->ToJson(parser, *node);
        }

    }

    static const int variantTypeNameCount = 18;
    static const char* variantTypeNameMap[variantTypeNameCount] =
    {
        "empty",        //AMF_VARIANT_EMPTY           = 0,
        "bool",         //AMF_VARIANT_BOOL            = 1,
        "int64",        //AMF_VARIANT_INT64           = 2,
        "double",       //AMF_VARIANT_DOUBLE          = 3,
        "rect",         //AMF_VARIANT_RECT            = 4,
        "size",         //AMF_VARIANT_SIZE            = 5,
        "point",        //AMF_VARIANT_POINT           = 6,
        "rate",         //AMF_VARIANT_RATE            = 7,
        "ratio",        //AMF_VARIANT_RATIO           = 8,
        "color",        //AMF_VARIANT_COLOR           = 9,
        "string",       //AMF_VARIANT_STRING          = 10,  // value is char*
        "wstring",      //AMF_VARIANT_WSTRING         = 11,  // value is wchar_t*
        "interface",    //AMF_VARIANT_INTERFACE       = 12,  // value is AMFInterface*
        "float",        //AMF_VARIANT_FLOAT           = 13,
        "fsize",        //AMF_VARIANT_FLOAT_SIZE      = 14,
        "fpoint2D",     //AMF_VARIANT_FLOAT_POINT2D   = 15,
        "fpoint3D",     //AMF_VARIANT_FLOAT_POINT3D   = 16,
        "fvect4",       //AMF_VARIANT_FLOAT_VECTOR4D  = 17
    };

    void amf::SetVariantValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const amf::AMFVariant& value)
    {
        amf::JSONParser::Node::Ptr node;
        amf::SetVariantToJSON(parser, &node, value);
        root->AddElement(name, node);
    }

    void amf::SetVariantToJSON(amf::JSONParser * parser, amf::JSONParser::Node ** node, const amf::AMFVariant & value)
    {
        parser->CreateNode(node);
        SetStringValue(parser, *node, "Type", variantTypeNameMap[value.type]);

        amf::JSONParser::Element::Ptr el;
        CreateVariantValue(parser, &el, value);
        (*node)->AddElement(TAG_JSON_VALUE, el);
    }

    void amf::CreateVariantValue(amf::JSONParser* parser, amf::JSONParser::Element** el, const amf::AMFVariant& value)
    {
        switch (value.type)
        {
        case amf::AMF_VARIANT_EMPTY:
            break;
        case amf::AMF_VARIANT_BOOL:
            CreateBoolValue(parser, (amf::JSONParser::Value**)el, value.boolValue);
            break;
        case amf::AMF_VARIANT_INT64:
            CreateInt64Value(parser, (amf::JSONParser::Value**)el, value.int64Value);
            break;
        case amf::AMF_VARIANT_FLOAT:
            CreateFloatValue(parser, (amf::JSONParser::Value**)el, value.ToFloat());
            break;
        case amf::AMF_VARIANT_DOUBLE:
            CreateDoubleValue(parser, (amf::JSONParser::Value**)el, value.ToDouble());
            break;
        case amf::AMF_VARIANT_STRING:
            CreateStringValue(parser, (amf::JSONParser::Value**)el, value.ToString().c_str());
            break;
        case amf::AMF_VARIANT_WSTRING:
            CreateStringValue(parser, (amf::JSONParser::Value**)el, value.ToString().c_str());
            break;
        case amf::AMF_VARIANT_RECT:
            CreateRectValue(parser, (amf::JSONParser::Array**)el, value.ToRect());
            break;
        case amf::AMF_VARIANT_SIZE:
            CreateSizeValue(parser, (amf::JSONParser::Array**)el, value.ToSize());
            break;
        case amf::AMF_VARIANT_POINT:
            CreatePointValue(parser, (amf::JSONParser::Array**)el, value.ToPoint());
            break;
        case amf::AMF_VARIANT_RATE:
            CreateRateValue(parser, (amf::JSONParser::Array**)el, value.ToRate());
            break;
        case amf::AMF_VARIANT_RATIO:
            CreateRatioValue(parser, (amf::JSONParser::Array**)el, value.ToRatio());
            break;
        case amf::AMF_VARIANT_COLOR:
            CreateColorValue(parser, (amf::JSONParser::Array**)el, value.ToColor());
            break;
        case amf::AMF_VARIANT_FLOAT_SIZE:
            CreateFloatSizeValue(parser, (amf::JSONParser::Array**)el, value.ToFloatSize());
            break;
        case amf::AMF_VARIANT_FLOAT_POINT2D:
            CreateFloatPoint2DValue(parser, (amf::JSONParser::Array**)el, value.ToFloatPoint2D());
            break;
        case amf::AMF_VARIANT_FLOAT_POINT3D:
            CreateFloatPoint3DValue(parser, (amf::JSONParser::Array**)el, value.ToFloatPoint3D());
            break;
        case amf::AMF_VARIANT_FLOAT_VECTOR4D:
            CreateFloatVector4DValue(parser, (amf::JSONParser::Array**)el, value.ToFloatVector4D());
            break;
        case amf::AMF_VARIANT_INTERFACE:
            CreateInterfaceValue(parser, (amf::JSONParser::Node**)el, value.ToInterface());
            break;
        default:
            break;
        }
    }

    bool amf::GetInt32Value(const amf::JSONParser::Node* root, const char *name, int32_t &val)
    {
        amf::JSONParser::Value::Ptr element(root->GetElementByName(name));
        return amf::GetInt32FromJSON(element, val);
    }

    bool amf::GetInt32FromJSON(const amf::JSONParser::Value* element, int32_t& val)
    {
        bool result = false;
        if (element != nullptr)
        {
            val = element->GetValueAsInt32();
            result = true;
        }
        return result;
    }

    bool amf::GetUInt32Value(const amf::JSONParser::Node* root, const char* name, uint32_t& val)
    {
        amf::JSONParser::Value::Ptr element(root->GetElementByName(name));
        return amf::GetUInt32FromJSON(element, val);
    }

    bool amf::GetUInt32FromJSON(const amf::JSONParser::Value* element, uint32_t& val)
    {
        bool result = false;
        if (element != nullptr)
        {
            val = element->GetValueAsUInt32();
            result = true;
        }
        return result;
    }

    bool amf::GetInt64Value(const amf::JSONParser::Node* root, const char* name, int64_t& val)
    {
        amf::JSONParser::Value::Ptr element(root->GetElementByName(name));
        return amf::GetInt64FromJSON(element, val);
    }

    bool amf::GetInt64FromJSON(const amf::JSONParser::Value* element, int64_t& val)
    {
        bool result = false;
        if (element != nullptr)
        {
            val = element->GetValueAsInt64();
            result = true;
        }
        return result;
    }

    bool amf::GetUInt64Value(const amf::JSONParser::Node* root, const char* name, uint64_t& val)
    {
        amf::JSONParser::Value::Ptr element(root->GetElementByName(name));
        return amf::GetUInt64FromJSON(element, val);
    }

    bool amf::GetUInt64FromJSON(const amf::JSONParser::Value* element, uint64_t& val)
    {
        bool result = false;
        if (element != nullptr)
        {
            val = element->GetValueAsUInt64();
            result = true;
        }
        return result;
    }

    bool amf::GetDoubleValue(const amf::JSONParser::Node* root, const char* name, double& val)
    {
        amf::JSONParser::Value::Ptr element(root->GetElementByName(name));
        return amf::GetDoubleFromJSON(element, val);
    }

    bool amf::GetDoubleFromJSON(const amf::JSONParser::Value * element, double & val)
    {
        bool result = false;
        if (element != nullptr)
        {
            val = element->GetValueAsDouble();
            result = true;
        }
        return result;
    }

    bool amf::GetFloatValue(const amf::JSONParser::Node* root, const char *name, float &val)
    {
        amf::JSONParser::Value::Ptr element(root->GetElementByName(name));
        return amf::GetFloatFromJSON(element, val);
    }

    bool amf::GetFloatFromJSON(const amf::JSONParser::Value* element, float& val)
    {
        bool result = false;
        if (element != nullptr)
        {
            val = element->GetValueAsFloat();
            result = true;
        }
        return result;
    }

    bool amf::GetFloatArray(const amf::JSONParser::Node* root, const char *name, float *val, size_t &size)
    {
        amf::JSONParser::Array::Ptr element(root->GetElementByName(name));
        return amf::GetFloatArrayFromJSON(element, val, size);
    }

    bool amf::GetFloatArrayFromJSON(const amf::JSONParser::Array* element, float* val, size_t& size)
    {
        bool result = false;
        if (element != nullptr)
        {
            size = AMF_MIN(element->GetElementCount(), size);
            for (size_t i = 0; i < size; i++)
            {

                val[i] = ((const amf::JSONParser::Value*)(element->GetElementAt(i)))->GetValueAsFloat();
            }
            result = true;
        }
        return result;
    }

    bool amf::GetDoubleArray(const amf::JSONParser::Node* root, const char* name, double* val, size_t& size)
    {
        amf::JSONParser::Array::Ptr element(root->GetElementByName(name));
        return amf::GetDoubleArrayFromJSON(element, val, size);
    }

    bool amf::GetDoubleArrayFromJSON(const amf::JSONParser::Array* element, double* val, size_t& size)
    {
        bool result = false;
        if (element != nullptr)
        {
            size = AMF_MIN(element->GetElementCount(), size);
            for (size_t i = 0; i < size; i++)
            {

                val[i] = ((const amf::JSONParser::Value*)(element->GetElementAt(i)))->GetValueAsDouble();
            }
            result = true;
        }
        return result;
    }

    bool amf::GetInt64Array(const amf::JSONParser::Node* root, const char *name, int64_t *val, size_t &size)
    {
        amf::JSONParser::Array::Ptr element(root->GetElementByName(name));
        return amf::GetInt64ArrayFromJSON(element, val, size);
    }

    bool amf::GetInt64ArrayFromJSON(const amf::JSONParser::Array * element, int64_t * val, size_t & size)
    {
        bool result = false;
        if (element != nullptr)
        {
            size = AMF_MIN(element->GetElementCount(), size);
            for (size_t i = 0; i < size; i++)
            {

                val[i] = ((const amf::JSONParser::Value*)(element->GetElementAt(i)))->GetValueAsInt64();
            }
            result = true;
        }
        return result;
    }

    bool amf::GetInt32Array(const amf::JSONParser::Node* root, const char* name, int32_t* val, size_t& size)
    {
        amf::JSONParser::Array::Ptr element(root->GetElementByName(name));
        return amf::GetInt32ArrayFromJSON(element, val, size);
    }

    bool amf::GetInt32ArrayFromJSON(const amf::JSONParser::Array* element, int32_t* val, size_t& size)
    {
        bool result = false;
        if (element != nullptr)
        {
            size = AMF_MIN(element->GetElementCount(), size);
            for (size_t i = 0; i < size; i++)
            {
                val[i] = ((const amf::JSONParser::Value*)(element->GetElementAt(i)))->GetValueAsInt32();
            }
            result = true;
        }
        return result;
    }

    bool amf::GetUInt32Array(const amf::JSONParser::Node* root, const char *name, uint32_t *val, size_t &size)
    {
        amf::JSONParser::Array::Ptr element(root->GetElementByName(name));
        return amf::GetUInt32ArrayFromJSON(element, val, size);
    }

    bool amf::GetUInt32ArrayFromJSON(const amf::JSONParser::Array * element, uint32_t * val, size_t & size)
    {
        bool result = false;
        if (element != nullptr)
        {
            size = AMF_MIN(element->GetElementCount(), size);
            for (size_t i = 0; i < size; i++)
            {

                val[i] = ((const amf::JSONParser::Value*)(element->GetElementAt(i)))->GetValueAsUInt32();
            }
            result = true;
        }
        return result;
    }

    bool amf::GetBoolValue(const amf::JSONParser::Node* root, const char* name, bool& val)
    {
        amf::JSONParser::Value::Ptr element(root->GetElementByName(name));
        return amf::GetBoolFromJSON(element, val);
    }

    bool amf::GetBoolFromJSON(const amf::JSONParser::Value* element, bool& val)
    {
        bool result = false;
        if (element != nullptr)
        {
            val = element->GetValueAsBool();
            result = true;
        }
        return result;
    }

    bool amf::GetSizeValue(const amf::JSONParser::Node* root, const char* name, AMFSize& val)
    {
        return amf::GetSizeFromJSON(root->GetElementByName(name), val);
    }

    bool amf::GetSizeFromJSON(const amf::JSONParser::Element* element, AMFSize& val)
    {
        int32_t arrayVal[2] = {};
        size_t size = amf_countof(arrayVal);
        amf::JSONParser::Array::Ptr array(const_cast<amf::JSONParser::Element*>(element));
        bool ret = (array != nullptr ? GetInt32ArrayFromJSON(array, arrayVal, size) : false);
        if (ret)
        {
            val.width = arrayVal[0];
            val.height = arrayVal[1];
        }
        return ret;
    }

    bool amf::GetPointValue(const amf::JSONParser::Node* root, const char* name, AMFPoint& val)
    {
        return amf::GetPointFromJSON(root->GetElementByName(name), val);
    }

    bool amf::GetPointFromJSON(const amf::JSONParser::Element* element, AMFPoint& val)
    {
        int32_t arrayVal[2];
        size_t size = amf_countof(arrayVal);
        amf::JSONParser::Array::Ptr array(const_cast<amf::JSONParser::Element*>(element));
        bool ret = (array != nullptr ? GetInt32ArrayFromJSON(array, arrayVal, size) : false);
        if (ret)
        {
            val.x = arrayVal[0];
            val.y = arrayVal[1];
        }
        return ret;
    }

    bool amf::GetRectValue(const amf::JSONParser::Node* root, const char* name, AMFRect& val)
    {
        return amf::GetRectFromJSON(root->GetElementByName(name), val);
    }

    bool amf::GetRectFromJSON(const amf::JSONParser::Element* element, AMFRect& val)
    {
        int32_t arrayVal[4];
        size_t size = amf_countof(arrayVal);
        amf::JSONParser::Array::Ptr array(const_cast<amf::JSONParser::Element*>(element));
        bool ret = (array != nullptr ? GetInt32ArrayFromJSON(array, arrayVal, size) : false);
        if (ret)
        {
            val.left = arrayVal[0];
            val.top = arrayVal[1];
            val.right = arrayVal[2];
            val.bottom = arrayVal[3];
        }
        return ret;
    }

    bool amf::GetRateValue(const amf::JSONParser::Node* root, const char* name, AMFRate& val)
    {
        return amf::GetRateFromJSON(root->GetElementByName(name), val);
    }

    bool amf::GetRateFromJSON(const amf::JSONParser::Element* element, AMFRate & val)
    {
        uint32_t arrayVal[2];
        size_t size = amf_countof(arrayVal);
        amf::JSONParser::Array::Ptr array(const_cast<amf::JSONParser::Element*>(element));
        bool ret = (array != nullptr ? GetUInt32ArrayFromJSON(array, arrayVal, size) : false);
        if (ret)
        {
            val.num = arrayVal[0];
            val.den = arrayVal[1];
        }
        return ret;
    }

    bool amf::GetRatioValue(const amf::JSONParser::Node* root, const char* name, AMFRatio& val)
    {
        return amf::GetRatioFromJSON(root->GetElementByName(name), val);
    }

    bool amf::GetRatioFromJSON(const amf::JSONParser::Element* element, AMFRatio & val)
    {
        uint32_t arrayVal[2];
        size_t size = amf_countof(arrayVal);
        amf::JSONParser::Array::Ptr array(const_cast<amf::JSONParser::Element*>(element));
        bool ret = (array != nullptr ? GetUInt32ArrayFromJSON(array, arrayVal, size) : false);
        if (ret)
        {
            val.num = arrayVal[0];
            val.den = arrayVal[1];
        }
        return ret;
    }

    bool amf::GetColorValue(const amf::JSONParser::Node* root, const char* name, AMFColor& val)
    {
        return amf::GetColorFromJSON(root->GetElementByName(name), val);
    }

    bool amf::GetColorFromJSON(const amf::JSONParser::Element* element, AMFColor & val)
    {
        uint32_t arrayVal[4];
        size_t size = amf_countof(arrayVal);
        amf::JSONParser::Array::Ptr array(const_cast<amf::JSONParser::Element*>(element));
        bool ret = (array != nullptr ? GetUInt32ArrayFromJSON(array, arrayVal, size) : false);
        if (ret)
        {
            val.a = static_cast<uint8_t>(arrayVal[0]);
            val.r = static_cast<uint8_t>(arrayVal[1]);
            val.g = static_cast<uint8_t>(arrayVal[2]);
            val.b = static_cast<uint8_t>(arrayVal[3]);
        }
        return ret;
    }

    bool amf::GetFloatSizeValue(const amf::JSONParser::Node* root, const char* name, AMFFloatSize& val)
    {
        return amf::GetFloatSizeFromJSON(root->GetElementByName(name), val);
    }

    bool amf::GetFloatSizeFromJSON(const amf::JSONParser::Element* element, AMFFloatSize & val)
    {
        float arrayVal[2] = {};
        size_t size = amf_countof(arrayVal);
        amf::JSONParser::Array::Ptr array(const_cast<amf::JSONParser::Element*>(element));
        bool ret = (array != nullptr ? GetFloatArrayFromJSON(array, arrayVal, size) : false);
        if (ret)
        {
            val.width = arrayVal[0];
            val.height = arrayVal[1];
        }
        return ret;
    }

    bool amf::GetFloatPoint2DValue(const amf::JSONParser::Node* root, const char* name, AMFFloatPoint2D& val)
    {
        return amf::GetFloatPoint2DFromJSON(root->GetElementByName(name), val);
    }

    bool amf::GetFloatPoint2DFromJSON(const amf::JSONParser::Element* element, AMFFloatPoint2D & val)
    {
        float arrayVal[2] = {};
        size_t size = amf_countof(arrayVal);
        amf::JSONParser::Array::Ptr array(const_cast<amf::JSONParser::Element*>(element));
        bool ret = (array != nullptr ? GetFloatArrayFromJSON(array, arrayVal, size) : false);
        if (ret)
        {
            val.x = arrayVal[0];
            val.y = arrayVal[1];
        }
        return ret;
    }

    bool amf::GetFloatPoint3DValue(const amf::JSONParser::Node* root, const char* name, AMFFloatPoint3D& val)
    {
        return amf::GetFloatPoint3DFromJSON(root->GetElementByName(name), val);
    }

    bool amf::GetFloatPoint3DFromJSON(const amf::JSONParser::Element* element, AMFFloatPoint3D & val)
    {
        float arrayVal[3] = {};
        size_t size = amf_countof(arrayVal);
        amf::JSONParser::Array::Ptr array(const_cast<amf::JSONParser::Element*>(element));
        bool ret = (array != nullptr ? GetFloatArrayFromJSON(array, arrayVal, size) : false);
        if (ret)
        {
            val.x = arrayVal[0];
            val.y = arrayVal[1];
            val.z = arrayVal[2];
        }
        return ret;
    }

    bool amf::GetFloatVector4DValue(const amf::JSONParser::Node* root, const char* name, AMFFloatVector4D& val)
    {
        return amf::GetFloatVector4DFromJSON(root->GetElementByName(name), val);
    }

    bool amf::GetFloatVector4DFromJSON(const amf::JSONParser::Element* element, AMFFloatVector4D & val)
    {
        float arrayVal[4] = {};
        size_t size = amf_countof(arrayVal);
        amf::JSONParser::Array::Ptr array(const_cast<amf::JSONParser::Element*>(element));
        bool ret = (array != nullptr ? GetFloatArrayFromJSON(array, arrayVal, size) : false);
        if (ret)
        {
            val.x = arrayVal[0];
            val.y = arrayVal[1];
            val.z = arrayVal[2];
            val.w = arrayVal[3];
        }
        return ret;
    }

    bool amf::GetStringValue(const amf::JSONParser::Node* root, const char *name, std::string &val)
    {
        amf::JSONParser::Value::Ptr element(root->GetElementByName(name));
        return amf::GetStringFromJSON(element, val);
    }

    bool amf::GetStringFromJSON(const amf::JSONParser::Value* element, std::string& val)
    {
        bool result = false;
        if (element != nullptr && element->IsNull() == false)
        {
            val = element->GetValue();
            result = true;
        }
        return result;
    }

    bool amf::GetInterfaceValue(const amf::JSONParser::Node* root, const char* name, AMFInterface* pVal)
    {
        return amf::GetInterfaceFromJSON(root->GetElementByName(name), pVal);
    }

    bool amf::GetInterfaceFromJSON(const amf::JSONParser::Element* element, AMFInterface* pVal)
    {
        const amf::JSONParser::Node* node = (const amf::JSONParser::Node*) element;
        AMFInterfaceJSONSerializablePtr p(pVal);
        bool result = false;
        if (p != nullptr)
        {
            p->FromJson(node);
            result = true;
        }

        return result;
    }

    bool amf::GetVariantValue(const amf::JSONParser::Node* root, const char *name, amf::AMFVariant& val)
    {
        amf::JSONParser::Node::Ptr element(root->GetElementByName(name));
        return amf::GetVariantFromJSON(element, val);
    }

    bool amf::GetVariantFromJSON(const amf::JSONParser::Node * element, amf::AMFVariant & val)
    {
        bool result = false;
        if (element != nullptr)
        {
            std::string propName, propType;
            if (GetStringValue(element, "Type", propType) == true)
            {
                for (int i = 0; i < variantTypeNameCount; i++)
                {
                    if (propType == variantTypeNameMap[i])
                    {
                        val.type = (AMF_VARIANT_TYPE)i;
                        result = true;
                        break;
                    }
                }

                if (result == true)
                {
                    result = GetVariantValueFromJSON(element->GetElementByName(TAG_JSON_VALUE), val);
                }
            }
        }
        return result;
    }

    //assumes val.type is pre-filled and valid
    bool amf::GetVariantValueFromJSON(const amf::JSONParser::Element* element, amf::AMFVariant& val)
    {
        bool result = false;
        switch (val.type)
        {
        case amf::AMF_VARIANT_EMPTY:
            break;
        case amf::AMF_VARIANT_BOOL:
            result = GetBoolFromJSON((const amf::JSONParser::Value*)element, val.boolValue);
            break;
        case amf::AMF_VARIANT_INT64:
            result = GetInt64FromJSON((const amf::JSONParser::Value*)element, val.int64Value);
            break;
        case amf::AMF_VARIANT_FLOAT:
            result = GetFloatFromJSON((const amf::JSONParser::Value*)element, val.floatValue);
            break;
        case amf::AMF_VARIANT_DOUBLE:
            result = GetDoubleFromJSON((const amf::JSONParser::Value*)element, val.doubleValue);
            break;
        case amf::AMF_VARIANT_STRING:
            {
                val.type = amf::AMF_VARIANT_EMPTY;
                std::string value;
                result = GetStringFromJSON((const amf::JSONParser::Value*)element, value);
                val = value.c_str();
            }
            break;
        case amf::AMF_VARIANT_WSTRING:
            {
                val.type = amf::AMF_VARIANT_EMPTY;
                std::string value;
                result = GetStringFromJSON((const amf::JSONParser::Value*)element, value);
                val = value.c_str();
                val.ChangeType(amf::AMF_VARIANT_WSTRING);
            }
            break;
        case amf::AMF_VARIANT_RECT:
            result = GetRectFromJSON(element, val.rectValue);
            break;
        case amf::AMF_VARIANT_SIZE:
            result = GetSizeFromJSON(element, val.sizeValue);
            break;
        case amf::AMF_VARIANT_POINT:
            result = GetPointFromJSON(element, val.pointValue);
            break;
        case amf::AMF_VARIANT_RATE:
            result = GetRateFromJSON(element, val.rateValue);
            break;
        case amf::AMF_VARIANT_RATIO:
            result = GetRatioFromJSON(element, val.ratioValue);
            break;
        case amf::AMF_VARIANT_COLOR:
            result = GetColorFromJSON(element, val.colorValue);
            break;
        case amf::AMF_VARIANT_FLOAT_SIZE:
            result = GetFloatSizeFromJSON(element, val.floatSizeValue);
            break;
        case amf::AMF_VARIANT_FLOAT_POINT2D:
            result = GetFloatPoint2DFromJSON(element, val.floatPoint2DValue);
            break;
        case amf::AMF_VARIANT_FLOAT_POINT3D:
            result = GetFloatPoint3DFromJSON(element, val.floatPoint3DValue);
            break;
        case amf::AMF_VARIANT_FLOAT_VECTOR4D:
            result = GetFloatVector4DFromJSON(element, val.floatVector4DValue);
            break;
        case amf::AMF_VARIANT_INTERFACE:
            result = GetInterfaceFromJSON(element, val.pInterface);
            break;
        default:
            break;
        }
        return result;
    }
}
