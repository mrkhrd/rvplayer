object FormDetails: TFormDetails
  Left = 899
  Top = 566
  AlphaBlend = True
  AlphaBlendValue = 220
  BorderStyle = bsNone
  Caption = 'Details'
  ClientHeight = 201
  ClientWidth = 296
  Color = clWhite
  Font.Charset = DEFAULT_CHARSET
  Font.Color = clWindowText
  Font.Height = -11
  Font.Name = 'MS Sans Serif'
  Font.Style = []
  OldCreateOrder = False
  OnCreate = FormCreate
  OnDestroy = FormDestroy
  OnMouseMove = FormMouseMove
  PixelsPerInch = 96
  TextHeight = 13
  object ShapeBorder: TShape
    Left = 168
    Top = 128
    Width = 112
    Height = 57
    OnMouseMove = FormMouseMove
  end
  object LabelSystemTimeCaption: TLabel
    Left = 8
    Top = 16
    Width = 63
    Height = 13
    Caption = 'System Time:'
  end
  object LabelSystemTime: TLabel
    Left = 72
    Top = 16
    Width = 57
    Height = 13
    Caption = 'SystemTime'
  end
  object LabelVideoTimeCaption: TLabel
    Left = 8
    Top = 30
    Width = 56
    Height = 13
    Caption = 'Video Time:'
  end
  object LabelVideoTime: TLabel
    Left = 72
    Top = 30
    Width = 50
    Height = 13
    Caption = 'VideoTime'
  end
  object LabelGPSXCaption: TLabel
    Left = 175
    Top = 34
    Width = 50
    Height = 13
    Caption = 'Longitude:'
  end
  object LabelGPSX: TLabel
    Left = 227
    Top = 34
    Width = 59
    Height = 13
    Caption = 'N50.123456'
  end
  object LabelGPSYCaption: TLabel
    Left = 183
    Top = 47
    Width = 41
    Height = 13
    Caption = 'Latitude:'
  end
  object LabelGPSY: TLabel
    Left = 227
    Top = 47
    Width = 64
    Height = 13
    Caption = 'E154.123456'
  end
  object LabelGPS: TLabel
    Left = 203
    Top = 21
    Width = 22
    Height = 13
    Caption = 'GPS'
  end
  object LabelGPSSpeed: TLabel
    Left = 227
    Top = 86
    Width = 40
    Height = 13
    Caption = '50 km/h'
  end
  object LabelGPSSpeedCaption: TLabel
    Left = 191
    Top = 86
    Width = 34
    Height = 13
    Caption = 'Speed:'
  end
  object LabelGPSAzimut: TLabel
    Left = 227
    Top = 73
    Width = 18
    Height = 13
    Caption = '360'
  end
  object LabelGPSAzimutCaption: TLabel
    Left = 183
    Top = 73
    Width = 39
    Height = 13
    Caption = 'Bearing:'
  end
  object LabelGPSTrack: TLabel
    Left = 227
    Top = 99
    Width = 29
    Height = 13
    Caption = '120 m'
  end
  object LabelGPSTrackCaption: TLabel
    Left = 191
    Top = 99
    Width = 31
    Height = 13
    Caption = 'Track:'
  end
  object LabelGSensor: TLabel
    Left = 19
    Top = 59
    Width = 44
    Height = 13
    Caption = 'G-Sensor'
  end
  object LabelGSensorXCaption: TLabel
    Left = 69
    Top = 47
    Width = 8
    Height = 13
    Caption = 'x:'
  end
  object LabelGSensorX: TLabel
    Left = 81
    Top = 47
    Width = 16
    Height = 13
    Caption = 'G-x'
  end
  object LabelGSensorYCaption: TLabel
    Left = 69
    Top = 59
    Width = 8
    Height = 13
    Caption = 'y:'
  end
  object LabelGSensorY: TLabel
    Left = 81
    Top = 59
    Width = 16
    Height = 13
    Caption = 'G-y'
  end
  object LabelGSensorZCaption: TLabel
    Left = 69
    Top = 71
    Width = 8
    Height = 13
    Caption = 'z:'
  end
  object LabelGSensorZ: TLabel
    Left = 81
    Top = 71
    Width = 16
    Height = 13
    Caption = 'G-z'
  end
  object Image: TImage
    Left = 32
    Top = 118
    Width = 225
    Height = 73
    OnMouseMove = FormMouseMove
  end
  object LabelGSensorGCaption: TLabel
    Left = 66
    Top = 83
    Width = 14
    Height = 13
    Caption = '|V|:'
  end
  object LabelGSensorG: TLabel
    Left = 81
    Top = 83
    Width = 17
    Height = 13
    Caption = 'G-g'
  end
  object LabelGSensorIndex: TLabel
    Left = 19
    Top = 71
    Width = 36
    Height = 13
    Caption = '[GSInd]'
  end
  object LabelGPSIndex: TLabel
    Left = 227
    Top = 21
    Width = 43
    Height = 13
    Caption = '[GPSInd]'
  end
  object LabelFileCaption: TLabel
    Left = 48
    Top = 2
    Width = 19
    Height = 13
    Caption = 'File:'
  end
  object LabelFile: TLabel
    Left = 72
    Top = 2
    Width = 42
    Height = 13
    Caption = 'LabelFile'
  end
  object LabelGraphicName: TLabel
    Left = 3
    Top = 99
    Width = 91
    Height = 13
    Caption = 'LabelGraphicName'
  end
  object LabelGPSAltitude: TLabel
    Left = 227
    Top = 60
    Width = 18
    Height = 13
    Caption = '123'
  end
  object LabelGPSAltitudeCaption: TLabel
    Left = 183
    Top = 60
    Width = 38
    Height = 13
    Caption = 'Altitude:'
  end
end
