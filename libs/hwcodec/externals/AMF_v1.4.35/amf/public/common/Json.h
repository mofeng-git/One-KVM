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

#pragma once 

#include "public/include/core/Interface.h"
#include "public/include/core/Variant.h"
#include <string>
#include <stdint.h>

namespace amf
{
    class JSONParser : public amf::AMFInterface
    {
    public:
        //-----------------------------------------------------------------------------------------
        enum Result
        {
            OK,
            MISSING_QUOTE,
            MISSING_BRACE,
            MISSING_BRACKET,
            MISSING_DELIMITER,
            MISSING_VALUE,
            UNEXPECTED_END,
            DUPLICATE_NAME,
            INVALID_ARG,
            INVALID_VALUE
        };
        //-----------------------------------------------------------------------------------------
        typedef amf::AMFInterfacePtr_T<JSONParser>  Ptr;
        AMF_DECLARE_IID(0x14aefb78, 0x80af, 0x4ee1, 0x82, 0x9f, 0xa2, 0xfc, 0xc7, 0xae, 0xab, 0x33)
        //-----------------------------------------------------------------------------------------
        class Error
        {
        public:
            Error(JSONParser::Result error) :
                m_Ofs(0),
                m_Error(error)
            {
            }

            Error(size_t ofs, JSONParser::Result error) :
                m_Ofs(ofs),
                m_Error(error)
            {
            }

            inline size_t GetOffset() const { return m_Ofs; }
            inline JSONParser::Result GetResult() const { return m_Error; }
            
        private:
            size_t  m_Ofs;
            JSONParser::Result   m_Error;
        };
        //-----------------------------------------------------------------------------------------
        struct OutputFormatDesc
        {

            bool    bHumanReadable;
            bool    bNewLineBeforeBrace;
            char    cOffsetWith;
            uint8_t nOffsetSize;
        };
        //-----------------------------------------------------------------------------------------
        class Element : public amf::AMFInterface
        {
        public:
            typedef amf::AMFInterfacePtr_T<Element>  Ptr;
            AMF_DECLARE_IID(0xd2d71993, 0xbbcb, 0x420f, 0xbc, 0xdd, 0xd8, 0xd6, 0xb6, 0x2e, 0x46, 0x5e)

            virtual Error Parse(const std::string& str, size_t start, size_t end) = 0;
            virtual std::string Stringify() const = 0;
            virtual std::string StringifyFormatted(const OutputFormatDesc& format, int indent) const = 0;
        };
        //-----------------------------------------------------------------------------------------
        class Value : public Element
        {
        public:
            typedef amf::AMFInterfacePtr_T<Value>  Ptr;
            AMF_DECLARE_IID(0xba0e44d4, 0xa487, 0x4d64, 0xa4, 0x94, 0x93, 0x9b, 0xfd, 0x76, 0x72, 0x32)

            virtual void                SetValue(const std::string& val) = 0;
            virtual void                SetValueAsInt32(int32_t val) = 0;
            virtual void                SetValueAsUInt32(uint32_t val) = 0;
            virtual void                SetValueAsInt64(int64_t val) = 0;
            virtual void                SetValueAsUInt64(uint64_t val) = 0;
            virtual void                SetValueAsDouble(double val) = 0;
            virtual void                SetValueAsFloat(float val) = 0;
            virtual void                SetValueAsBool(bool val) = 0;
            virtual void                SetValueAsTime(time_t date, bool utc) = 0;
            virtual void                SetToNull() = 0;

            virtual const std::string&  GetValue() const = 0;
            virtual int32_t             GetValueAsInt32() const = 0;
            virtual uint32_t            GetValueAsUInt32() const = 0;
            virtual int64_t             GetValueAsInt64() const = 0;
            virtual uint64_t            GetValueAsUInt64() const = 0;
            virtual double              GetValueAsDouble() const = 0;
            virtual float               GetValueAsFloat() const = 0;
            virtual bool                GetValueAsBool() const = 0;
            virtual time_t              GetValueAsTime() const = 0;
            virtual bool                IsNull() const = 0;
        };
        //-----------------------------------------------------------------------------------------
        class Node : public Element
        {
        public:
            typedef amf::AMFInterfacePtr_T<Node>  Ptr;
            AMF_DECLARE_IID(0x6623d6b8, 0x533d, 0x4824, 0x9d, 0x3b, 0x45, 0x1a, 0xa8, 0xc3, 0x7b, 0x5d)

            virtual size_t GetElementCount() const = 0;
            virtual JSONParser::Element* GetElementByName(const std::string& name) const = 0;
            virtual JSONParser::Result AddElement(const std::string& name, Element* element) = 0;
            virtual JSONParser::Element* GetElementAt(size_t idx, std::string& name) const = 0;
        };
        //-----------------------------------------------------------------------------------------
        class Array : public Element
        {
        public:
            typedef amf::AMFInterfacePtr_T<Array>  Ptr;
            AMF_DECLARE_IID(0x8c066a6d, 0xb377, 0x44e8, 0x8c, 0xf5, 0xf8, 0xbf, 0x88, 0x85, 0xbb, 0xe9)

            virtual size_t GetElementCount() const = 0;
            virtual JSONParser::Element* GetElementAt(size_t idx) const = 0;
            virtual void AddElement(Element* element) = 0;
        };
        //-----------------------------------------------------------------------------------------
        virtual Result Parse(const std::string& str, Node** root) = 0;  //  Parse a JSON string into a tree of DOM elements
        virtual std::string Stringify(const Node* root) const = 0;  //  Convert a DOM to a JSON string
        virtual std::string StringifyFormatted(const Node* root, const OutputFormatDesc& format, int indent = 0) const = 0;

        virtual Result CreateNode(Node** node) const = 0;
        virtual Result CreateValue(Value** value) const = 0;
        virtual Result CreateArray(Array** array) const = 0;

        virtual size_t GetLastErrorOffset() const = 0;  //  Returns the offset of the last syntax error (same as what is passed in the exception if thrown)
    };

    extern "C"
    {
        // Helpers
        #define TAG_JSON_VALUE "Val"

        void SetBoolValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, bool val);
        void CreateBoolValue(amf::JSONParser* parser, amf::JSONParser::Value** node, bool val);
        bool GetBoolValue(const amf::JSONParser::Node* root, const char* name, bool& val);
        bool GetBoolFromJSON(const amf::JSONParser::Value* element, bool& val);

        void SetDoubleValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, double val);
        void CreateDoubleValue(amf::JSONParser* parser, amf::JSONParser::Value** node, double val);
        bool GetDoubleValue(const amf::JSONParser::Node* root, const char* name, double& val);
        bool GetDoubleFromJSON(const amf::JSONParser::Value* element, double& val);

        void SetFloatValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, float val);
        void CreateFloatValue(amf::JSONParser* parser, amf::JSONParser::Value** node, float val);
        bool GetFloatValue(const amf::JSONParser::Node* root, const char* name, float& val);
        bool GetFloatFromJSON(const amf::JSONParser::Value* element, float& val);

        void SetInt64Value(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name,  int64_t val);
        void CreateInt64Value(amf::JSONParser* parser, amf::JSONParser::Value** node, const int64_t val);
        bool GetInt64Value(const amf::JSONParser::Node* root, const char* name, int64_t& val);
        bool GetInt64FromJSON(const amf::JSONParser::Value* element, int64_t& val);

        void SetUInt64Value(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name,  uint64_t val);
        bool GetUInt64Value(const amf::JSONParser::Node* root, const char* name, uint64_t& val);
        bool GetUInt64FromJSON(const amf::JSONParser::Value* element, uint64_t& val);

        void SetInt32Value(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name,  int32_t val);
        bool GetInt32Value(const amf::JSONParser::Node* root, const char* name, int32_t& val);
        bool GetInt32FromJSON(const amf::JSONParser::Value* element, int32_t& val);

        void SetUInt32Value(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name,  uint32_t val);
        bool GetUInt32Value(const amf::JSONParser::Node* root, const char* name, uint32_t& val);
        bool GetUInt32FromJSON(const amf::JSONParser::Value* element, uint32_t& val);

        void SetUInt32Array(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const uint32_t* val, size_t size);
        void CreateUInt32Array(amf::JSONParser* parser, amf::JSONParser::Array** array, const uint32_t* val, size_t size);
        bool GetUInt32Array(const amf::JSONParser::Node* root, const char* name, uint32_t* val, size_t& size);
        bool GetUInt32ArrayFromJSON(const amf::JSONParser::Array* element, uint32_t* val, size_t& size);

        void SetInt32Array(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const int32_t* val, size_t size);
        void CreateInt32Array(amf::JSONParser* parser, amf::JSONParser::Array** array, const int32_t* val, size_t size);
        bool GetInt32Array(const amf::JSONParser::Node* root, const char* name, int32_t* val, size_t& size);
        bool GetInt32ArrayFromJSON(const amf::JSONParser::Array* element, int32_t* val, size_t& size);

        void SetInt64Array(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const int64_t* val, size_t size);
        bool GetInt64Array(const amf::JSONParser::Node* root, const char* name, int64_t* val, size_t& size);
        bool GetInt64ArrayFromJSON(const amf::JSONParser::Array* element, int64_t* val, size_t& size);

        void SetFloatArray(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const float* val, size_t size);
        void CreateFloatArray(amf::JSONParser* parser, amf::JSONParser::Array** array, const float* val, size_t size);
        bool GetFloatArray(const amf::JSONParser::Node* root, const char* name, float* val, size_t& size);
        bool GetFloatArrayFromJSON(const amf::JSONParser::Array* element, float* val, size_t& size);

        void SetDoubleArray(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const double* val, size_t size);
        bool GetDoubleArray(const amf::JSONParser::Node* root, const char* name, double* val, size_t& size);
        bool GetDoubleArrayFromJSON(const amf::JSONParser::Array* element, double* val, size_t& size);

        void SetSizeValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFSize& val);
        void CreateSizeValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFSize& val);
        bool GetSizeValue(const amf::JSONParser::Node* root, const char* name, AMFSize& val);
        bool GetSizeFromJSON(const amf::JSONParser::Element* element, AMFSize& val);

        void SetRectValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFRect& val);
        void CreateRectValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFRect& val);
        bool GetRectValue(const amf::JSONParser::Node* root, const char* name, AMFRect& val);
        bool GetRectFromJSON(const amf::JSONParser::Element* element, AMFRect& val);

        void SetPointValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFPoint& val);
        void CreatePointValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFPoint& val);
        bool GetPointValue(const amf::JSONParser::Node* root, const char* name, AMFPoint& val);
        bool GetPointFromJSON(const amf::JSONParser::Element* element, AMFPoint& val);

        void SetRateValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFRate& val);
        void CreateRateValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFRate& val);
        bool GetRateValue(const amf::JSONParser::Node* root, const char* name, AMFRate& val);
        bool GetRateFromJSON(const amf::JSONParser::Element* element, AMFRate& val);

        void SetRatioValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFRatio& val);
        void CreateRatioValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFRatio& val);
        bool GetRatioValue(const amf::JSONParser::Node* root, const char* name, AMFRatio& val);
        bool GetRatioFromJSON(const amf::JSONParser::Element* element, AMFRatio& val);

        void SetColorValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFColor& val);
        void CreateColorValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFColor& val);
        bool GetColorValue(const amf::JSONParser::Node* root, const char* name, AMFColor& val);
        bool GetColorFromJSON(const amf::JSONParser::Element* element, AMFColor& val);

        void SetFloatSizeValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFFloatSize& val);
        void CreateFloatSizeValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFFloatSize& val);
        bool GetFloatSizeValue(const amf::JSONParser::Node* root, const char* name, AMFFloatSize& val);
        bool GetFloatSizeFromJSON(const amf::JSONParser::Element* element, AMFFloatSize& val);

        void SetFloatPoint2DValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFFloatPoint2D& val);
        void CreateFloatPoint2DValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFFloatPoint2D& val);
        bool GetFloatPoint2DValue(const amf::JSONParser::Node* root, const char* name, AMFFloatPoint2D& val);
        bool GetFloatPoint2DFromJSON(const amf::JSONParser::Element* element, AMFFloatPoint2D& val);

        void SetFloatPoint3DValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFFloatPoint3D& val);
        void CreateFloatPoint3DValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFFloatPoint3D& val);
        bool GetFloatPoint3DValue(const amf::JSONParser::Node* root, const char* name, AMFFloatPoint3D& val);
        bool GetFloatPoint3DFromJSON(const amf::JSONParser::Element* element, AMFFloatPoint3D& val);

        void SetFloatVector4DValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const AMFFloatVector4D& val);
        void CreateFloatVector4DValue(amf::JSONParser* parser, amf::JSONParser::Array** array, const AMFFloatVector4D& val);
        bool GetFloatVector4DValue(const amf::JSONParser::Node* root, const char* name, AMFFloatVector4D& val);
        bool GetFloatVector4DFromJSON(const amf::JSONParser::Element* element, AMFFloatVector4D& val);

        void SetStringValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const std::string& val);
        void CreateStringValue(amf::JSONParser* parser, amf::JSONParser::Value** node, const std::string& val);
        bool GetStringValue(const amf::JSONParser::Node* root, const char* name, std::string& val);
        bool GetStringFromJSON(const amf::JSONParser::Value* element, std::string& val);

        void SetInterfaceValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, /*const*/ AMFInterface* pVal);
        void CreateInterfaceValue(amf::JSONParser* parser, amf::JSONParser::Node** node, /*const*/ AMFInterface* pval);
        bool GetInterfaceValue(const amf::JSONParser::Node* root, const char* name, AMFInterface* ppVal);
        bool GetInterfaceFromJSON(const amf::JSONParser::Element* element, AMFInterface* ppVal);

        void SetVariantValue(amf::JSONParser* parser, amf::JSONParser::Node* root, const char* name, const amf::AMFVariant& value);
        void SetVariantToJSON(amf::JSONParser* parser, amf::JSONParser::Node** node, const amf::AMFVariant& value);
        bool GetVariantValue(const amf::JSONParser::Node* root, const char* name, amf::AMFVariant& val);
        bool GetVariantFromJSON(const amf::JSONParser::Node* element, amf::AMFVariant& val);

        // variant value only;  variant type assumed to be pre-set
        void CreateVariantValue(amf::JSONParser* parser, amf::JSONParser::Element** el, const amf::AMFVariant& value);
        bool GetVariantValueFromJSON(const amf::JSONParser::Element* element, amf::AMFVariant& val);
    }

    class AMFInterfaceJSONSerializable : public amf::AMFInterface
    {
    public:
        // {EC40A26C-1345-4281-9B6C-362DDD6E05B5}
        AMF_DECLARE_IID(0xec40a26c, 0x1345, 0x4281, 0x9b, 0x6c, 0x36, 0x2d, 0xdd, 0x6e, 0x5, 0xb5)
        //
        virtual AMF_RESULT AMF_STD_CALL ToJson(amf::JSONParser* parser, amf::JSONParser::Node* node) const = 0;

        //
        virtual AMF_RESULT AMF_STD_CALL FromJson(const amf::JSONParser::Node* node) = 0;
    };
    typedef AMFInterfacePtr_T<AMFInterfaceJSONSerializable> AMFInterfaceJSONSerializablePtr;
}

extern "C"
{
    AMF_RESULT AMF_CDECL_CALL CreateJSONParser(amf::JSONParser** parser);
    #define AMF_JSON_PARSER_FACTORY "CreateJSONParser"
}
