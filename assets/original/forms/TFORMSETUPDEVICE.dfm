object FormSetupDevice: TFormSetupDevice
  Left = 192
  Top = 124
  AutoSize = True
  BorderIcons = [biSystemMenu]
  BorderStyle = bsSingle
  BorderWidth = 10
  Caption = 'FormSetupDevice'
  ClientHeight = 219
  ClientWidth = 325
  Color = clBtnFace
  Font.Charset = DEFAULT_CHARSET
  Font.Color = clWindowText
  Font.Height = -11
  Font.Name = 'MS Sans Serif'
  Font.Style = []
  OldCreateOrder = False
  OnCreate = FormCreate
  OnShow = FormShow
  PixelsPerInch = 96
  TextHeight = 13
  object LabelFrameSize: TLabel
    Left = 31
    Top = 54
    Width = 55
    Height = 13
    Caption = 'Frame Size:'
  end
  object LabelDevice: TLabel
    Left = 69
    Top = 6
    Width = 24
    Height = 13
    Caption = 'Disk:'
  end
  object LabelTimeZone: TLabel
    Left = 28
    Top = 86
    Width = 54
    Height = 13
    Caption = 'Time Zone:'
  end
  object LabelXCaption: TLabel
    Left = 80
    Top = 114
    Width = 7
    Height = 13
    Caption = 'X'
  end
  object LabelGSensor: TLabel
    Left = 20
    Top = 130
    Width = 47
    Height = 13
    Alignment = taCenter
    Caption = 'G-Sensor:'
  end
  object LabelX: TLabel
    Left = 307
    Top = 114
    Width = 12
    Height = 13
    Caption = '10'
  end
  object LabelYCaption: TLabel
    Left = 80
    Top = 139
    Width = 7
    Height = 13
    Caption = 'Y'
  end
  object LabelY: TLabel
    Left = 307
    Top = 139
    Width = 12
    Height = 13
    Caption = '10'
  end
  object LabelZCaption: TLabel
    Left = 80
    Top = 164
    Width = 7
    Height = 13
    Caption = 'Z'
  end
  object LabelZ: TLabel
    Left = 307
    Top = 164
    Width = 12
    Height = 13
    Caption = '10'
  end
  object Bevel1: TBevel
    Left = 36
    Top = 34
    Width = 265
    Height = 9
    Shape = bsTopLine
  end
  object ImageDod: TImage
    Left = 0
    Top = 0
    Width = 50
    Height = 23
    AutoSize = True
    Transparent = True
  end
  object ComboBoxDevice: TComboBox
    Left = 96
    Top = 2
    Width = 229
    Height = 21
    Style = csDropDownList
    ItemHeight = 13
    TabOrder = 0
    OnChange = ComboBoxDeviceChange
    Items.Strings = (
      'T:\'
    )
  end
  object ComboBoxFrameSize: TComboBox
    Left = 96
    Top = 50
    Width = 229
    Height = 22
    Style = csDropDownList
    Enabled = False
    Font.Charset = DEFAULT_CHARSET
    Font.Color = clWindowText
    Font.Height = -11
    Font.Name = 'Courier New'
    Font.Style = []
    ItemHeight = 14
    ParentFont = False
    TabOrder = 1
    OnChange = ComboBoxFrameSizeChange
    Items.Strings = (
      '1080P   1920x1080   30fps'
      '1080P   1440x1080   30fps'
      ' 720P   1280x720    30fps'
      ' WVGA    848x480    60fps'
      ' WVGA    848x480    30fps'
      '  VGA    640x480    30fps'
      ' QVGA    320x240    30fps'
      '< Custom >'
    )
  end
  object GSensorX: TTrackBar
    Left = 88
    Top = 112
    Width = 217
    Height = 25
    Enabled = False
    PageSize = 1
    TabOrder = 3
    ThumbLength = 15
    OnChange = GSensorXChange
  end
  object GSensorY: TTrackBar
    Left = 88
    Top = 137
    Width = 217
    Height = 25
    Enabled = False
    PageSize = 1
    TabOrder = 4
    ThumbLength = 15
    OnChange = GSensorXChange
  end
  object GSensorZ: TTrackBar
    Left = 88
    Top = 162
    Width = 217
    Height = 25
    Enabled = False
    PageSize = 1
    TabOrder = 5
    ThumbLength = 15
    OnChange = GSensorXChange
  end
  object ButtonWrite: TButton
    Left = 68
    Top = 194
    Width = 75
    Height = 25
    Caption = 'Write'
    Default = True
    Enabled = False
    TabOrder = 6
    OnClick = ButtonWriteClick
  end
  object ButtonCancel: TButton
    Left = 156
    Top = 194
    Width = 75
    Height = 25
    Cancel = True
    Caption = 'Cancel'
    TabOrder = 7
    OnClick = ButtonCancelClick
  end
  object ComboBoxTimeZone: TComboBox
    Left = 96
    Top = 82
    Width = 81
    Height = 22
    Style = csDropDownList
    Font.Charset = DEFAULT_CHARSET
    Font.Color = clWindowText
    Font.Height = -11
    Font.Name = 'Courier New'
    Font.Style = []
    ItemHeight = 14
    ParentFont = False
    TabOrder = 2
  end
end
