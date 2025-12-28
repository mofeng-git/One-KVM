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

#pragma once

#include "Json.h"
#include "InterfaceImpl.h"
#include <map>
#include <ctime>

namespace amf
{
    //-----------------------------------------------------------------------------------------
    class JSONParserImpl : 
        public AMFInterfaceImpl<JSONParser>
    {
    public:
        //-----------------------------------------------------------------------------------------
        class ElementHelper
        {
        protected:
            ElementHelper();

            Error CreateElement(const std::string& str, size_t start, size_t& valueStart, size_t& valueEnd, JSONParser::Element** val);
            size_t FindClosure(const std::string& str, char opener, char closer, size_t start);
            void InsertTabs(std::string& target, int count, const OutputFormatDesc& format) const;
        protected:
        };
        //-----------------------------------------------------------------------------------------
        class ValueImpl : 
            public AMFInterfaceImpl<JSONParser::Value>,
            public ElementHelper
        {
        public:
            ValueImpl();

            AMF_BEGIN_INTERFACE_MAP
                AMF_INTERFACE_ENTRY(JSONParser::Element)
                AMF_INTERFACE_ENTRY(JSONParser::Value)
            AMF_END_INTERFACE_MAP


            virtual JSONParser::Error Parse(const std::string& str, size_t start, size_t end);
            virtual std::string Stringify() const;
            virtual std::string StringifyFormatted(const OutputFormatDesc& format, int indent) const;

            virtual void                SetValue(const std::string& val);
            virtual void                SetValueAsInt32(int32_t val);
            virtual void                SetValueAsUInt32(uint32_t val);
            virtual void                SetValueAsInt64(int64_t val);
            virtual void                SetValueAsUInt64(uint64_t val);
            virtual void                SetValueAsDouble(double val);
            virtual void                SetValueAsFloat(float val);
            virtual void                SetValueAsBool(bool val);
            virtual void                SetValueAsTime(time_t date, bool utc);
            virtual void                SetToNull();

            virtual const std::string&  GetValue() const;
            virtual int32_t             GetValueAsInt32() const;
            virtual uint32_t            GetValueAsUInt32() const;
            virtual int64_t             GetValueAsInt64() const;
            virtual uint64_t            GetValueAsUInt64() const;
            virtual double              GetValueAsDouble() const;
            virtual float               GetValueAsFloat() const;
            virtual bool                GetValueAsBool() const;
            virtual time_t              GetValueAsTime() const;
            virtual bool                IsNull() const;

        private:
            enum VALUE_TYPE
            {
                VT_Unknown  = 0,
                VT_Null     = 1,
                VT_Bool     = 2,
                VT_String   = 3,
                VT_Numeric  = 4,
            };
            VALUE_TYPE  m_eType;
            std::string m_Value;
        };
        //-----------------------------------------------------------------------------------------
        class NodeImpl : 
            public AMFInterfaceImpl<JSONParser::Node>,
            public ElementHelper
        {
        public:
            typedef std::map<std::string, Element::Ptr> ElementMap;

            AMF_BEGIN_INTERFACE_MAP
                AMF_INTERFACE_ENTRY(JSONParser::Element)
                AMF_INTERFACE_ENTRY(JSONParser::Node)
            AMF_END_INTERFACE_MAP

            NodeImpl();

            virtual JSONParser::Error Parse(const std::string& str, size_t start, size_t end);
            virtual std::string Stringify() const;
            virtual std::string StringifyFormatted(const OutputFormatDesc& format, int indent) const;
            

            virtual size_t GetElementCount() const;
            virtual JSONParser::Element* GetElementByName(const std::string& name) const;
            virtual JSONParser::Result AddElement(const std::string& name, JSONParser::Element* element);
            virtual JSONParser::Element* GetElementAt(size_t idx, std::string& name) const;

            const ElementMap& GetElements() const { return m_Elements; }

        private:
            ElementMap m_Elements;
        };
        //-----------------------------------------------------------------------------------------
        class ArrayImpl : 
            public AMFInterfaceImpl<JSONParser::Array>,
            public ElementHelper
        {
        public:
            typedef std::vector<Element::Ptr>   ElementVector;

            AMF_BEGIN_INTERFACE_MAP
                AMF_INTERFACE_ENTRY(JSONParser::Element)
                AMF_INTERFACE_ENTRY(JSONParser::Array)
            AMF_END_INTERFACE_MAP

            ArrayImpl();

            virtual JSONParser::Error Parse(const std::string& str, size_t start, size_t end);
            virtual std::string Stringify() const;
            virtual std::string StringifyFormatted(const OutputFormatDesc& format, int indent) const;

            virtual size_t GetElementCount() const;
            virtual JSONParser::Element* GetElementAt(size_t idx) const;
            virtual void AddElement(Element* element);

        private:
            ElementVector m_Elements;
        };
        //-----------------------------------------------------------------------------------------
        JSONParserImpl();

        virtual JSONParser::Result Parse(const std::string& str, Node** root);
        virtual std::string Stringify(const Node* root) const;
        virtual std::string StringifyFormatted(const Node* root, const OutputFormatDesc& format, int indent) const;

        virtual size_t GetLastErrorOffset() const;

        virtual Result CreateNode(Node** node) const;
        virtual Result CreateValue(Value** value) const;
        virtual Result CreateArray(Array** array) const;

    private:
        size_t              m_LastErrorOfs;
    };
}
